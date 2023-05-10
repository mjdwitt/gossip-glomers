use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::ser::Serialize;
use serde_json::Value as Json;
use tailsome::*;
use tap::prelude::*;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use tracing::*;

use crate::message::{Body, Message, MsgId, NodeId};
use crate::node::error::Error;
use crate::node::signal::{Signal, TimedSignal};

#[async_trait]
pub trait Rpc {
    /// Attempts to build and send a reply message, giving up if it fails anywhere.
    async fn send_reply<B: Serialize + Debug + Send + Sync>(
        &self,
        src: NodeId,
        dest: NodeId,
        in_reply_to: MsgId,
        body: B,
    ) -> Result<(), Box<dyn StdError + Send + Sync + 'static>>;
    /// Builds and sends a message. Returns an error if it cannot build a message. Retries sends
    /// forever in a spawned task until notified of a successful reply via [`notify_ok`]. The
    /// default [`crate::node::Node::run`] loop will notify it's registered rpc system of all
    /// replies for you.
    async fn send_rpc<B: Serialize + Debug + Send + Sync>(
        &self,
        src: NodeId,
        dest: NodeId,
        body: B,
    ) -> Result<(), Box<dyn StdError + Send + Sync + 'static>>;
    /// Notifies the rpc system that a successful reply to the `source_id` message was seen.
    async fn notify_ok(&self, source_id: MsgId, ok: Message<Json>) -> Option<MsgId>;
    /// Notifies the rpc system that an error reply to the `source_id` message was seen.
    async fn notify_error(&self, source_id: MsgId, err: Message<Error>) -> Option<MsgId>;
}

pub type ProdRpc<O> = RpcWriter<O, TimedSignal>;

pub struct RpcWriter<O, R> {
    out: Arc<Mutex<O>>,
    next_id: Arc<AtomicU64>,
    pending: Arc<RwLock<HashMap<MsgId, Json>>>,
    tasks: Arc<Mutex<JoinSet<Result<(), std::io::Error>>>>,
    resend: R,
}

impl<O> ProdRpc<O> {
    pub fn new(out: Arc<Mutex<O>>) -> Self {
        Self {
            out,
            next_id: Default::default(),
            pending: Default::default(),
            tasks: Default::default(),
            resend: TimedSignal::new(Duration::from_millis(500)),
        }
    }
}

impl<O, R: Clone> Clone for RpcWriter<O, R> {
    fn clone(&self) -> Self {
        Self {
            out: self.out.clone(),
            next_id: self.next_id.clone(),
            pending: self.pending.clone(),
            resend: self.resend.clone(),
            tasks: self.tasks.clone(),
        }
    }
}

#[async_trait]
impl<O, R> Rpc for RpcWriter<O, R>
where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
    R: Signal + Clone + Send + Sync + 'static,
{
    async fn send_reply<B: Serialize + Debug + Send + Sync>(
        &self,
        src: NodeId,
        dest: NodeId,
        in_reply_to: MsgId,
        body: B,
    ) -> Result<(), Box<dyn StdError + Send + Sync + 'static>> {
        let (_, reply) = self
            .serialize_message(src, dest, Some(in_reply_to), body)
            .await?;
        debug!(?reply, "send_reply");
        self.write_message(&reply).await?.into_ok()
    }

    async fn send_rpc<B: Serialize + Debug + Send + Sync>(
        &self,
        src: NodeId,
        dest: NodeId,
        body: B,
    ) -> Result<(), Box<dyn StdError + Send + Sync + 'static>> {
        let (msg_id, msg) = self.serialize_message(src, dest, None, body).await?;
        {
            let mut pending = self.pending.write().await;
            pending.insert(msg_id, msg.clone());
            debug!(?pending, "send_rpc");
        }
        self.ensure_sent(msg_id, msg).await;
        Ok(())
    }

    async fn notify_ok(&self, source_id: MsgId, ok: Message<Json>) -> Option<MsgId> {
        self.pending
            .write()
            .await
            .tap_dbg(|pending| debug!(?pending, "notify_ok"))
            .remove(&source_id)
            .tap_some(|msg| debug!(?ok, ?msg, "rpc succeeded"))
            .tap_none(|| debug!(?ok, "received an ok reply but have no pending source rpc"))
            .map(|_| source_id)
    }

    async fn notify_error(&self, source_id: MsgId, err: Message<Error>) -> Option<MsgId> {
        self.pending
            .read()
            .await
            .tap_dbg(|pending| debug!(?pending, "notify_ok"))
            .get(&source_id)
            .tap_some(|msg| error!(?err, ?msg, "received an error in reply to an rpc"))
            .tap_none(|| {
                error!(
                    ?err,
                    "received an error reply but have no pending source rpc"
                )
            })
            .map(|_| source_id)
    }
}

impl<O, R> RpcWriter<O, R>
where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
    R: Signal + Clone + Send + Sync + 'static,
{
    async fn serialize_message<B: Serialize + Debug + Send + Sync>(
        &self,
        src: NodeId,
        dest: NodeId,
        in_reply_to: Option<MsgId>,
        body: B,
    ) -> Result<(MsgId, Json), serde_json::Error> {
        let msg_id = self.next_id.fetch_add(1, Ordering::SeqCst).into();
        let msg = serde_json::to_value(&Message {
            src,
            dest,
            body: Body {
                msg_id,
                in_reply_to,
                body,
            },
        })?;
        (msg_id, msg).into_ok()
    }

    async fn write_message(&self, msg: &Json) -> Result<(), std::io::Error> {
        let bytes = &[msg.to_string().as_bytes(), b"\n"].concat();
        self.out
            .lock()
            .await
            .write_all(bytes)
            .await
            .tap_err(|write_error| error!(?write_error, ?msg, "failed to write rpc to output"))?
            .into_ok()
    }

    async fn ensure_sent(&self, msg_id: MsgId, msg: Json) {
        let res = self.write_message(&msg).await;
        debug!(?msg_id, ?msg, "sending guaranteed rpc");
        self.tasks
            .lock()
            .await
            .spawn(self.clone().resend_loop(msg_id, msg, res));
    }

    async fn resend_loop(
        self,
        msg_id: MsgId,
        msg: Json,
        mut res: Result<(), std::io::Error>,
    ) -> Result<(), std::io::Error> {
        loop {
            self.resend.signal().await;

            if res.is_err() || self.pending.read().await.contains_key(&msg_id) {
                debug!(?msg_id, ?msg, "resending un-ack'd rpc");
                res = self.write_message(&msg).await;
            } else {
                return Ok(());
            }
        }
    }
}

impl<O: Debug, R> RpcWriter<O, R> {
    #[cfg(test)]
    async fn graceful_stop(self) {
        let RpcWriter { tasks, .. } = self;
        {
            let mut tasks = tasks.lock().await;
            while let Some(_) = tasks.join_next().await {}
        }
        Arc::try_unwrap(tasks).unwrap();
    }

    #[cfg(test)]
    async fn abort(self) -> O {
        let mut tasks = self.tasks.lock().await;
        tasks.abort_all();
        while let Some(_) = tasks.join_next().await {}
        Arc::try_unwrap(self.out).unwrap().into_inner()
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufRead;

    use rstest::rstest;
    use serde_json::json;
    use tokio::io::{AsyncBufReadExt, BufReader};

    use super::*;
    use crate::node::signal::Never;

    #[tokio::test]
    async fn send_rpc_writes_message_with_id() {
        let (rx, tx) = tokio::io::duplex(64);
        let rpc = RpcWriter {
            out: Arc::new(Mutex::new(tx)),
            next_id: Default::default(),
            pending: Default::default(),
            tasks: Default::default(),
            resend: Never,
        };

        rpc.send_rpc("n1".into(), "n2".into(), ()).await.unwrap();

        let mut rx = BufReader::new(rx);
        let mut buf = String::new();
        rx.read_line(&mut buf).await.unwrap();
        assert_eq!(
            serde_json::from_str::<Json>(&buf).unwrap(),
            json!({
                "src": "n1",
                "dest": "n2",
                "body": { "msg_id": 0 },
            }),
        );
    }

    #[tokio::test]
    async fn replies_and_rpcs_use_incrementing_message_ids() {
        let out = Arc::new(Mutex::new(Vec::new()));

        let rpc = RpcWriter {
            out,
            next_id: Arc::new(AtomicU64::new(0)),
            pending: Default::default(),
            tasks: Default::default(),
            resend: Never,
        };

        for _ in 0..10 {
            rpc.send_rpc("n1".into(), "n2".into(), ()).await.unwrap();
            rpc.send_reply("n1".into(), "n2".into(), 7.into(), ())
                .await
                .unwrap();
        }

        assert!(rpc
            .abort()
            .await
            .as_slice()
            .pipe(BufRead::lines)
            .inspect(|line| debug!(?line))
            .map(|line| -> u64 {
                serde_json::from_str::<Message<()>>(&line.unwrap())
                    .unwrap()
                    .body
                    .msg_id
                    .into()
            })
            .eq(0..20));
    }

    fn msg(id: impl Into<MsgId>, re: impl Into<MsgId>) -> Message<Json> {
        Message {
            src: "c1".into(),
            dest: "n1".into(),
            body: Body {
                msg_id: id.into(),
                in_reply_to: Some(re.into()),
                body: json!(null),
            },
        }
    }

    fn err(id: impl Into<MsgId>, re: impl Into<MsgId>, text: impl Into<String>) -> Message<Error> {
        Message {
            src: "c1".into(),
            dest: "n1".into(),
            body: Body {
                msg_id: id.into(),
                in_reply_to: Some(re.into()),
                body: Error {
                    code: 13,
                    text: text.into(),
                },
            },
        }
    }

    #[rstest]
    async fn notify_with_none_pending() {
        let rpc = RpcWriter::new(Arc::new(Mutex::new(tokio::io::sink())));
        assert_eq!(rpc.notify_ok(1.into(), msg(2, 1)).await, None);
        assert_eq!(rpc.notify_error(1.into(), err(2, 1, "error")).await, None);
    }

    #[rstest]
    async fn notify_ok_clears_pending() {
        let pending: HashMap<MsgId, Json> = [(1.into(), json!(null))].into_iter().collect();
        let pending = Arc::new(RwLock::new(pending));
        let rpc = RpcWriter {
            out: Arc::new(Mutex::new(tokio::io::sink())),
            next_id: Default::default(),
            pending: pending.clone(),
            tasks: Default::default(),
            resend: Never,
        };

        assert!(pending.read().await.contains_key(&1.into()));
        assert_eq!(rpc.notify_ok(1.into(), msg(2, 1)).await, Some(1.into()));
        assert!(!pending.read().await.contains_key(&1.into()));
    }

    #[rstest]
    async fn notify_err_does_not_clear_pending() {
        let pending: HashMap<MsgId, Json> = [(1.into(), json!(null))].into_iter().collect();
        let pending = Arc::new(RwLock::new(pending));
        let rpc = RpcWriter {
            out: Arc::new(Mutex::new(tokio::io::sink())),
            next_id: Default::default(),
            pending: pending.clone(),
            tasks: Default::default(),
            resend: Never,
        };

        assert!(pending.read().await.contains_key(&1.into()));
        assert_eq!(
            rpc.notify_error(1.into(), err(2, 1, "error")).await,
            Some(1.into())
        );
        assert!(pending.read().await.contains_key(&1.into()));
    }

    #[tokio::test]
    async fn rpcs_are_resent_until_notified_ok() {
        let (read, write) = tokio::io::duplex(4096);
        let (resend, signal) = tokio::sync::mpsc::channel::<()>(1);

        let rpc = RpcWriter {
            out: Arc::new(Mutex::new(write)),
            next_id: Default::default(),
            pending: Default::default(),
            tasks: Default::default(),
            resend: Arc::new(Mutex::new(signal)),
        };

        rpc.send_rpc("n1".into(), "n2".into(), ()).await.unwrap();
        assert_eq!(rpc.pending.read().await.len(), 1);

        let mut lines = BufReader::new(read).lines();
        let _: String = lines.next_line().await.unwrap().unwrap();

        resend.send(()).await.unwrap();
        let _: String = lines.next_line().await.unwrap().unwrap();

        rpc.notify_ok(0.into(), msg(2, 1)).await;
        assert_eq!(rpc.pending.read().await.len(), 0);

        resend.send(()).await.unwrap();
        rpc.graceful_stop().await;
        assert_eq!(lines.next_line().await.unwrap(), None);
    }
}
