use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use tap::prelude::*;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use tracing::*;

use crate::handler::{ErasedHandler, Handler, State};
use crate::message::{Error, Headers, Message, Request, Response};

pub mod init;

type RouteMap<S> = HashMap<String, Arc<dyn ErasedHandler<S>>>;
type Routes<S> = Arc<RwLock<RouteMap<S>>>;

#[derive(Clone)]
pub struct Node<S> {
    ids: State<init::Ids>,
    state: State<S>,
    handlers: Routes<S>,
}

#[derive(Default)]
pub struct NodeBuilder<S> {
    handlers: RouteMap<S>,
}

impl<S> NodeBuilder<S> {
    pub fn handle<Fut, Req, Res>(
        mut self,
        type_: impl Into<String>,
        handler: impl Handler<Fut, Req, Res, S> + 'static,
    ) -> Self
    where
        S: Send + Sync + 'static,
        Fut: Future<Output = Res> + Send + 'static,
        Req: Request + 'static,
        Res: Response + 'static,
    {
        let handler: Box<dyn Handler<Fut, Req, Res, S>> = Box::new(handler);
        self.handlers.insert(type_.into(), Arc::new(handler));
        self
    }

    pub fn with_state(self, state: State<S>) -> Node<S> {
        Node {
            state,
            ids: Default::default(),
            handlers: Arc::new(RwLock::new(self.handlers)),
        }
    }
}

impl<S: Default + Send + Sync + 'static> Node<S> {
    pub fn new() -> NodeBuilder<S> {
        NodeBuilder::default()
    }

    pub async fn run<I, O>(self, i: I, o: O) -> Result<(), tokio::task::JoinError>
    where
        I: AsyncRead + Send + Sync + Unpin,
        O: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let o = Arc::new(Mutex::new(o));
        let mut lines = BufReader::new(i).lines();
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
            if result.is_ok() {
                result = res;
            }
        }

        result
    }
}

async fn run<O: AsyncWrite + Unpin, S: Send + Sync + 'static>(
    out: Arc<Mutex<O>>,
    handlers: Routes<S>,
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

    let mut raw_out = match handler.clone().call(state, raw).await {
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

    raw_out.push(b'\n');
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
    async fn node_run() {
        let ids: Arc<RwLock<init::Ids>> = Default::default();
        Node::new()
            .handle("init", init::init)
            .with_state(ids.clone())
            .run(
                br#"{"src":"c1","dest":"n1","body":{"type":"init","node_id":"n1","node_ids":["n1","n2","n3"]}}"#
                    .as_slice(),
                tokio::io::stderr())
            .await
            .unwrap();

        let ids = Arc::try_unwrap(ids).unwrap().into_inner();
        assert_eq!(ids.id, "n1");
        assert_eq!(ids.ids, vec!["n1", "n2", "n3"]);
    }
}
