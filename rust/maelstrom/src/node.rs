#![allow(dead_code)] // TODO: remove this

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use tap::prelude::*;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use tracing::*;

use crate::handler::{ErasedHandler, Handler, State};
use crate::message::{Error, Headers, Message};

pub mod init;

type RequestMapper<S> = Arc<RwLock<HashMap<String, Arc<dyn ErasedHandler<S>>>>>;

#[derive(Clone, Default)]
pub struct Node<S: Default> {
    state: State<S>,
    handlers: RequestMapper<S>,
}

impl<S: Default + Send + Sync + 'static> Node<S> {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn handle<Fut, Req, Res>(
        self,
        type_: impl Into<String>,
        handler: impl Handler<Fut, Req, Res, S> + 'static,
    ) -> Self
    where
        S: Send + Sync + 'static,
        Fut: Future<Output = Res> + Send + 'static,
        Req: DeserializeOwned + Send + 'static,
        Res: Serialize + Send + 'static,
    {
        let handler: Box<dyn Handler<Fut, Req, Res, S>> = Box::new(handler);
        self.handlers
            .write()
            // TODO: lift this into some builder class that is not thread-safe and does not
            // use an Arc<RwLock<_>> around the handlers map.
            .await
            .insert(type_.into(), Arc::new(handler));
        self
    }

    pub async fn run<I, O>(self, i: I, o: O) -> Result<(), tokio::task::JoinError>
    where
        I: AsyncBufRead + Send + Sync + Unpin,
        O: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let o = Arc::new(Mutex::new(o));
        let mut lines = i.lines();
        let mut requests = JoinSet::new();

        while let Ok(Some(line)) = lines.next_line().await {
            debug!(?line, "received request");
            requests.spawn(run(
                o.clone(),
                self.handlers.clone(),
                self.state.clone(),
                line,
            ));
        }

        let mut result = Ok(());
        while let Some(res) = requests.join_next().await {
            debug!(?res, "completed request");
            if result.is_ok() {
                result = res;
            }
        }

        result
    }
}

async fn run<O: AsyncWrite + Unpin, S: Send + Sync + 'static>(
    out: Arc<Mutex<O>>,
    handlers: RequestMapper<S>,
    state: State<S>,
    raw: String,
) {
    let headers: Message<Headers> = serde_json::from_str(&raw)
        .tap_err(|deserialization_error| {
            error!(
                ?deserialization_error,
                "failed to deserialize message headers",
            )
        })
        .unwrap();

    let handlers = handlers.read().await;
    let handler = handlers
        .get(&headers.body.type_)
        .tap_none(|| error!(?headers, "no handler found for message"))
        .expect("no handler found for message");

    let raw_out = match handler.clone().call(state, raw).await {
        Err(err) => {
            let err = Error {
                headers: Headers {
                    type_: "error".into(),
                    in_reply_to: headers.body.msg_id.map(|id| id + 1),
                    ..Headers::default()
                },
                code: 13, // crash: indefinite
                text: err.to_string(),
            };

            serde_json::to_vec(&err)
                .tap_err(|serialization_error| {
                    error!(
                        ?err,
                        ?serialization_error,
                        "failed to serialize Error response to json",
                    )
                })
                .unwrap()
        }
        Ok(response) => response,
    };

    out.lock()
        .await
        .write_all(&raw_out)
        .await
        .tap_err(|write_error| {
            error!(?write_error, ?raw_out, "failed to write response to output")
        })
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn node_run() -> Result<(), tokio::task::JoinError> {
        let out = Vec::new();
        Node::new()
            .handle("init", init::init)
            .await
            .run(
                br#"{"src": "c1", "dest": "n1", "body": {"type": "init", "msg_id": 1, "node_id": "n1", "node_ids": ["n1", "n2", "n3"]}}"#
                    .as_slice(),
                out,
            )
            .await?;

        Ok(())
    }
}
