use std::collections::HashMap;
use std::error::Error as StdError;
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

pub struct Rpc<O> {
    out: Arc<Mutex<O>>,

    next_id: Arc<AtomicU64>,
    pending: Arc<RwLock<HashMap<MsgId, Vec<u8>>>>,
}

impl<O> Clone for Rpc<O> {
    fn clone(&self) -> Self {
        Self {
            out: self.out.clone(),
            next_id: self.next_id.clone(),
            pending: self.pending.clone(),
        }
    }
}

impl<O: AsyncWrite + Send + Sync + Unpin + 'static> Rpc<O> {
    pub fn new(out: Arc<Mutex<O>>) -> Self {
        Self {
            out,
            next_id: Default::default(),
            pending: Default::default(),
        }
    }

    pub async fn send<B: Serialize + std::fmt::Debug>(
        &self,
        src: NodeId,
        dest: NodeId,
        body: B,
    ) -> Result<(), Box<dyn StdError + Send + Sync + 'static>> {
        let msg_id = self.next_id.fetch_add(1, Ordering::SeqCst).into();
        let mut msg = serde_json::to_vec(&Message {
            src,
            dest,
            body: Body {
                msg_id: Some(msg_id),
                in_reply_to: None,
                body,
            },
        })?;
        msg.push(b'\n');

        self.pending.write().await.insert(msg_id, msg.clone());

        tokio::spawn(self.clone().ensure_sent(msg_id, msg));
        Ok(())
    }

    async fn ensure_sent(self, msg_id: MsgId, msg: Vec<u8>) -> Result<(), std::io::Error> {
        loop {
            let res = self
                .out
                .lock()
                .await
                .write_all(&msg)
                .await
                .tap_err(|write_error| {
                    error!(?write_error, ?msg, "failed to write rpc to output")
                })?
                .into_ok::<std::io::Error>();

            if res.is_err() || self.pending.read().await.contains_key(&msg_id) {
                tokio::time::sleep(Duration::from_millis(500)).await;
            } else {
                break;
            }
        }

        Ok(())
    }

    pub async fn notify_ok(&self, source_id: MsgId, ok: Message<Json>) {
        match self.pending.write().await.remove(&source_id) {
            Some(msg) => {
                debug!(?ok, ?msg, "rpc succeeded");
            }
            None => debug!(?ok, "received an ok reply but have no pending source rpc"),
        }
    }

    pub async fn notify_error(&self, source_id: MsgId, err: Message<Error>) {
        match self.pending.read().await.get(&source_id) {
            Some(msg) => error!(?err, ?msg, "received an error in reply to an rpc"),
            None => error!(
                ?err,
                "received an error reply but have no pending source rpc"
            ),
        }
    }
}
