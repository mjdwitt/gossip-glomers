use std::error::Error;
use std::sync::Arc;

use serde::ser::Serialize;
use tailsome::*;
use tap::prelude::*;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::*;

use crate::message::Message;

pub struct Rpc<O> {
    out: Arc<Mutex<O>>,
}

impl<O> Clone for Rpc<O> {
    fn clone(&self) -> Self {
        Self {
            out: self.out.clone(),
        }
    }
}

impl<O: AsyncWrite + Unpin> Rpc<O> {
    pub fn new(out: Arc<Mutex<O>>) -> Self {
        Self { out }
    }

    pub async fn send<B: Serialize + std::fmt::Debug>(
        &self,
        msg: Message<B>,
    ) -> Result<(), Box<dyn Error>> {
        let mut msg = serde_json::to_vec(&msg)?;
        msg.push(b'\n');
        self.out
            .lock()
            .await
            .write_all(&msg)
            .await
            .tap_err(|write_error| error!(?write_error, ?msg, "failed to write rpc to output"))?
            .into_ok()
    }
}
