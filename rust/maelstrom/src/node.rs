use std::collections::HashMap;
use std::error::Error as StdError;
use std::future::Future;
use std::sync::Arc;

use futures::FutureExt;
use serde_json::json;
use tailsome::*;
use tap::prelude::*;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader};
use tokio::sync::oneshot::Sender;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use tracing::*;

use crate::handler::{ErasedHandler, Handler};
use crate::message::{Message, Request, Response, Type};

pub mod error;
pub mod init;
pub mod rpc;
pub mod state;

use error::Error;
use rpc::{Rpc, RpcWriter};
use state::{FromRef, RefInner, State};

type RouteMap<O, S> = HashMap<String, Arc<dyn ErasedHandler<O, S>>>;
type Routes<O, S> = Arc<RwLock<RouteMap<O, S>>>;

pub struct Node<O: AsyncWrite, S: Clone + FromRef<State<S>>> {
    ids: Arc<Mutex<Option<Sender<init::Ids>>>>,
    state: State<S>,
    handlers: Routes<O, S>,
}

#[derive(Default)]
pub struct NodeBuilder<O, S> {
    handlers: RouteMap<O, S>,
}

impl<O, S> NodeBuilder<O, S>
where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
    S: Clone + FromRef<State<S>>,
{
    fn new() -> Self {
        Self {
            handlers: Default::default(),
        }
    }

    pub fn handle<T, Req, Res, Fut>(
        mut self,
        type_: impl Into<String>,
        handler: impl Handler<T, S, O, Req, Res, Fut> + 'static,
    ) -> Self
    where
        T: 'static,
        S: Send + Sync + 'static,
        Fut: Future<Output = Res> + Send + 'static,
        Req: Request + 'static,
        Res: Response + 'static,
    {
        let handler: Box<dyn Handler<T, S, O, Req, Res, Fut>> = Box::new(handler);
        self.handlers.insert(type_.into(), Arc::new(handler));
        self
    }

    pub fn with_state(self, app: S) -> Node<O, S> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        Node {
            state: State {
                ids: rx.shared(),
                app,
            },
            ids: Arc::new(Mutex::new(Some(tx))),
            handlers: Arc::new(RwLock::new(self.handlers)),
        }
    }
}

impl<O, S> Node<O, S>
where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
    S: FromRef<State<S>> + Clone + Default + Send + Sync + 'static,
{
    pub fn builder() -> NodeBuilder<O, S> {
        NodeBuilder::new()
    }

    pub async fn run<I>(&self, i: I, o: O) -> Result<(), tokio::task::JoinError>
    where
        I: AsyncRead + Send + Sync + Unpin,
    {
        let o = Arc::new(Mutex::new(o));
        let mut lines = BufReader::new(i).lines();
        let mut requests = JoinSet::new();

        while let Ok(Some(line)) = lines.next_line().await {
            debug!(?line, "received request");
            requests.spawn(run(
                self.handlers.clone(),
                self.state.clone(),
                self.ids.clone(),
                RpcWriter::new(o.clone()),
                line,
            ));
        }

        let mut result = Ok(());
        while let Some(res) = requests.join_next().await {
            if result.is_ok() {
                if let Err(ref err) = res {
                    error!(?err, "request task failed");
                }
                result = res;
            }
        }

        result
    }
}

async fn run<O, S>(
    handlers: Routes<O, S>,
    state: State<S>,
    ids: Arc<Mutex<Option<Sender<init::Ids>>>>,
    rpc: RpcWriter<O>,
    raw: String,
) where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
    S: FromRef<State<S>> + Clone + Send + Sync + 'static,
{
    let headers: Message<Type> = serde_json::from_str(&raw)
        .tap_err(|deserialization_error| {
            error!(
                ?deserialization_error,
                "failed to deserialize message headers",
            )
        })
        .unwrap();

    let reply = match headers.body.body.type_.as_str() {
        "init" => init(ids, raw).await,
        "error" => error(rpc.clone(), raw).await,
        _ => {
            if let Some(source_id) = headers.body.in_reply_to {
                if let Ok(msg) = serde_json::from_str(&raw) {
                    rpc.notify_ok(source_id, msg).await;
                }
            }

            let handlers = handlers.read().await;
            let handler = handlers
                .get(&headers.body().type_)
                .tap_none(|| error!(?headers, "no handler found for message"))
                .expect("no handler found for message");
            handler
                .clone()
                .call(rpc.clone(), state.ids().await, state.inner(), raw)
                .await
        }
    }
    .unwrap_or_else(|err| {
        serde_json::to_value(&Error {
            code: 13, // crash: indefinite
            text: err.to_string(),
        })
        .unwrap()
    });

    // Some handlers may return an empty body, indicating that they have no reply to send.
    if null_body(&reply) {
        return;
    }

    rpc.send_reply(headers.dest, headers.src, headers.body.msg_id, &reply)
        .await
        .tap_err(|write_error| error!(?write_error, ?reply, "failed to write response to output"))
        .unwrap()
}

async fn init(
    ids: Arc<Mutex<Option<Sender<init::Ids>>>>,
    raw: String,
) -> Result<serde_json::Value, Box<dyn StdError>> {
    let req: Message<init::Init> = serde_json::from_str(&raw)?;
    serde_json::to_value(&init::init(ids, req.body.body).await)?.into_ok()
}

async fn error<O>(rpc: RpcWriter<O>, raw: String) -> Result<serde_json::Value, Box<dyn StdError>>
where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
{
    error::error(rpc, serde_json::from_str(&raw)?).await;
    Ok(json!(null))
}

fn null_body(raw_out: &serde_json::Value) -> bool {
    raw_out == &json!(null)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn default_node_handles_init() {
        let node = Node::builder().with_state(());
        node.run(
            serde_json::to_vec(&json!({
                "src": "c1",
                "dest": "n1",
                "body": {
                    "msg_id": 1,
                    "type": "init",
                    "node_id": "n1",
                    "node_ids": ["n1", "n2", "n3"],
                },
            }))
            .unwrap()
            .as_slice(),
            tokio::io::stderr(),
        )
        .await
        .unwrap();

        let ids: init::Ids = node.state.ids().await;
        assert_eq!(ids.id, "n1");
        assert_eq!(ids.ids, vec!["n1", "n2", "n3"]);
    }
}
