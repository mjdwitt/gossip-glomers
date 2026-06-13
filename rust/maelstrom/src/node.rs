use std::collections::HashMap;
use std::error::Error as StdError;
use std::future::Future;
use std::sync::Arc;

use futures::FutureExt;
use serde_json::json;
use serde_json::Value as Json;
use tailsome::*;
use tap::prelude::*;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader};
use tokio::sync::oneshot::Sender;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use tracing::*;

use crate::handler::{ErasedHandler, Handler};
use crate::message::{Message, MsgId, NodeId, Request, Response, Type};

pub mod error;
pub mod init;
pub mod rpc;
pub mod signal;
pub mod state;

use error::Error;
use rpc::{ProdRpc, Rpc};
use state::{FromRef, RefInner, State};

type RouteMap<R, S> = HashMap<String, Arc<dyn ErasedHandler<R, S>>>;
type Routes<R, S> = Arc<RwLock<RouteMap<R, S>>>;

pub struct Node<R: Rpc, S: Clone + FromRef<State<S>>> {
    ids: Arc<Mutex<Option<Sender<init::Ids>>>>,
    state: State<S>,
    handlers: Routes<R, S>,
}

#[derive(Default)]
pub struct NodeBuilder<R, S> {
    handlers: RouteMap<R, S>,
}

impl<R, S> NodeBuilder<R, S>
where
    R: Rpc + Clone + Send + Sync + Unpin + 'static,
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
        handler: impl Handler<T, S, R, Req, Res, Fut> + 'static,
    ) -> Self
    where
        T: 'static,
        S: Send + Sync + 'static,
        Fut: Future<Output = Res> + Send + 'static,
        Req: Request + 'static,
        Res: Response + 'static,
    {
        let handler: Box<dyn Handler<T, S, R, Req, Res, Fut>> = Box::new(handler);
        self.handlers.insert(type_.into(), Arc::new(handler));
        self
    }

    pub fn with_state(self, app: S) -> Node<R, S> {
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

impl<O, S> Node<ProdRpc<O>, S>
where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
    S: FromRef<State<S>> + Clone + Default + Send + Sync + 'static,
{
    pub fn builder() -> NodeBuilder<ProdRpc<O>, S> {
        NodeBuilder::new()
    }

    pub async fn run<I>(&self, i: I, o: O) -> Result<(), tokio::task::JoinError>
    where
        I: AsyncRead + Send + Sync + Unpin,
    {
        let o = Arc::new(Mutex::new(o));
        let rpc = ProdRpc::new(o.clone());
        let mut lines = BufReader::new(i).lines();
        let mut requests = JoinSet::new();

        while let Ok(Some(line)) = lines.next_line().await {
            debug!(?line, "received request");
            requests.spawn(run(
                self.handlers.clone(),
                self.state.clone(),
                self.ids.clone(),
                rpc.clone(),
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

async fn run<R, S>(
    handlers: Routes<R, S>,
    state: State<S>,
    ids: Arc<Mutex<Option<Sender<init::Ids>>>>,
    rpc: R,
    raw: String,
) where
    R: Rpc + Clone + Send + Sync + Unpin + 'static,
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

    if headers.headers.body.type_ == "batch" {
        dispatch_batch(handlers, state, rpc, raw).await;
        return;
    }

    let reply = match headers.headers.body.type_.as_str() {
        "init" => init(ids, raw).await,
        "error" => error(rpc.clone(), raw).await,
        _ => {
            if let Some(source_id) = headers.headers.in_reply_to {
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

    rpc.send_reply(headers.dest, headers.src, headers.headers.msg_id, &reply)
        .await
        .tap_err(|write_error| error!(?write_error, ?reply, "failed to write response to output"))
        .unwrap()
}

async fn dispatch_batch<R, S>(
    handlers: Routes<R, S>,
    state: State<S>,
    rpc: R,
    raw: String,
) where
    R: Rpc + Clone + Send + Sync + Unpin + 'static,
    S: FromRef<State<S>> + Clone + Send + Sync + 'static,
{
    let full: Json = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            error!(?e, "failed to deserialize batch envelope");
            return;
        }
    };
    let src: NodeId = match serde_json::from_value(full["src"].clone()) {
        Ok(v) => v,
        Err(e) => {
            error!(?e, "batch envelope missing src");
            return;
        }
    };
    let dest: NodeId = match serde_json::from_value(full["dest"].clone()) {
        Ok(v) => v,
        Err(e) => {
            error!(?e, "batch envelope missing dest");
            return;
        }
    };
    let bodies = full["body"]["bodies"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut replies: Vec<(MsgId, Json)> = Vec::new();
    for inner in bodies {
        if let Some(reply) =
            dispatch_inner(&handlers, &state, &rpc, &src, &dest, inner.clone()).await
        {
            if let Some(msg_id) = inner.get("msg_id").and_then(|v| {
                serde_json::from_value::<MsgId>(v.clone()).ok()
            }) {
                replies.push((msg_id, reply));
            }
        }
    }

    if let Err(e) = rpc.send_replies(dest, src, replies).await {
        error!(?e, "failed to write batched replies");
    }
}

async fn dispatch_inner<R, S>(
    handlers: &Routes<R, S>,
    state: &State<S>,
    rpc: &R,
    src: &NodeId,
    dest: &NodeId,
    inner_body: Json,
) -> Option<Json>
where
    R: Rpc + Clone + Send + Sync + Unpin + 'static,
    S: FromRef<State<S>> + Clone + Send + Sync + 'static,
{
    // If the inner body is a reply, notify the rpc system. We still fall through to the
    // handler dispatch because handlers may be registered for *_ok types.
    if let Some(source_id) = inner_body
        .get("in_reply_to")
        .and_then(|v| serde_json::from_value::<MsgId>(v.clone()).ok())
    {
        let synthetic = json!({
            "src": src,
            "dest": dest,
            "body": inner_body.clone(),
        });
        if let Ok(msg) = serde_json::from_value::<Message<Json>>(synthetic) {
            rpc.notify_ok(source_id, msg).await;
        }
    }

    let inner_type = inner_body.get("type").and_then(|t| t.as_str())?.to_string();
    let synthetic_raw = json!({
        "src": src,
        "dest": dest,
        "body": inner_body,
    })
    .to_string();

    let handlers_guard = handlers.read().await;
    let handler = handlers_guard.get(&inner_type).cloned();
    drop(handlers_guard);

    let handler = handler?;

    let reply = handler
        .call(rpc.clone(), state.ids().await, state.inner(), synthetic_raw)
        .await
        .unwrap_or_else(|err| {
            serde_json::to_value(&Error {
                code: 13,
                text: err.to_string(),
            })
            .unwrap()
        });

    if null_body(&reply) {
        None
    } else {
        Some(reply)
    }
}

async fn init(
    ids: Arc<Mutex<Option<Sender<init::Ids>>>>,
    raw: String,
) -> Result<serde_json::Value, Box<dyn StdError>> {
    let req: Message<init::Init> = serde_json::from_str(&raw)?;
    serde_json::to_value(init::init(ids, req.headers.body).await)?.into_ok()
}

async fn error<R>(rpc: R, raw: String) -> Result<serde_json::Value, Box<dyn StdError>>
where
    R: Rpc + Send + Sync + Unpin + 'static,
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
