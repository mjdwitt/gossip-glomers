use std::collections::HashMap;
use std::error::Error as StdError;
use std::future::Future;
use std::sync::Arc;

use tailsome::*;
use tap::prelude::*;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use tracing::*;

use crate::handler::{ErasedHandler, Handler, State};
use crate::message::{Error, Headers, Message, Request, Response, Type};

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
    pub fn builder() -> NodeBuilder<S> {
        NodeBuilder::default()
    }

    pub async fn run<I, O>(&self, i: I, o: O) -> Result<(), tokio::task::JoinError>
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
                self.ids.clone(),
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
    ids: State<init::Ids>,
    raw: String,
) {
    let headers: Message<Type> = serde_json::from_str(&raw)
        .tap_err(|deserialization_error| {
            error!(
                ?deserialization_error,
                "failed to deserialize message headers",
            )
        })
        .unwrap();

    let mut raw_out = {
        let res = if &headers.body.type_ == "init" {
            init(ids, raw).await
        } else {
            let handlers = handlers.read().await;
            let handler = handlers
                .get(&headers.body.type_)
                .tap_none(|| error!(?headers, "no handler found for message"))
                .expect("no handler found for message");
            handler.clone().call(state, raw).await
        };

        match res {
            Err(err) => {
                let err = Error {
                    headers: Headers {
                        in_reply_to: headers.body.headers.msg_id.map(|id| id + 1),
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
        }
    };

    // Some handlers may return `()`, which gets serialized to "null". Don't print those.
    if raw_out == b"null" {
        return;
    }

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

async fn init(ids: State<init::Ids>, raw: String) -> Result<Vec<u8>, Box<dyn StdError>> {
    let req: Message<init::Init> = serde_json::from_str(&raw)?;
    let res = Message {
        src: req.dest,
        dest: req.src,
        body: init::init(ids, req.body).await,
    };
    serde_json::to_vec(&res)?.into_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_node_handles_init() {
        let node = Node::builder().with_state(Arc::new(RwLock::new(())));
        node.run(
                br#"{"src":"c1","dest":"n1","body":{"type":"init","node_id":"n1","node_ids":["n1","n2","n3"]}}"#
                    .as_slice(),
                tokio::io::stderr())
            .await
            .unwrap();

        let ids = node.ids.read().await;
        assert_eq!(ids.id, "n1");
        assert_eq!(ids.ids, vec!["n1", "n2", "n3"]);
    }
}
