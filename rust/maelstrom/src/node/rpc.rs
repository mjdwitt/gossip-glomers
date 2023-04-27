use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::ser::Serialize;
use serde_json::Value as Json;
use tailsome::*;
use tap::prelude::*;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock};
use tracing::*;

use crate::message::{Body, Message, MsgId, NodeId};
use crate::node::error::Error;

#[async_trait::async_trait]
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

pub struct RpcWriter<O> {
    out: Arc<Mutex<O>>,

    next_id: Arc<AtomicU64>,
    pending: Arc<RwLock<HashMap<MsgId, Json>>>,
}

impl<O> Clone for RpcWriter<O> {
    fn clone(&self) -> Self {
        Self {
            out: self.out.clone(),
            next_id: self.next_id.clone(),
            pending: self.pending.clone(),
        }
    }
}

#[async_trait::async_trait]
impl<O: AsyncWrite + Send + Sync + Unpin + 'static> Rpc for RpcWriter<O> {
    async fn send_reply<B: Serialize + Debug + Send + Sync>(
        &self,
        src: NodeId,
        dest: NodeId,
        in_reply_to: MsgId,
        body: B,
    ) -> Result<(), Box<dyn StdError + Send + Sync + 'static>> {
        let (_, msg) = self
            .serialize_message(src, dest, Some(in_reply_to), body)
            .await?;
        self.write_message(&msg).await?.into_ok()
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
        tokio::spawn(self.clone().ensure_sent(msg_id, msg));
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

impl<O: AsyncWrite + Send + Sync + Unpin + 'static> RpcWriter<O> {
    pub fn new(out: Arc<Mutex<O>>) -> Self {
        Self {
            out,
            next_id: Default::default(),
            pending: Default::default(),
        }
    }

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

    async fn ensure_sent(self, msg_id: MsgId, msg: Json) -> Result<(), std::io::Error> {
        loop {
            if self.write_message(&msg).await.is_err()
                || self.pending.read().await.contains_key(&msg_id)
            {
                tokio::time::sleep(Duration::from_millis(500)).await;
            } else {
                break;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;

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
        };

        assert!(pending.read().await.contains_key(&1.into()));
        assert_eq!(
            rpc.notify_error(1.into(), err(2, 1, "error")).await,
            Some(1.into())
        );
        assert!(pending.read().await.contains_key(&1.into()));
    }
}
