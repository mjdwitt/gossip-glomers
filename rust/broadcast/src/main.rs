use std::collections::HashMap;

use maelstrom::prelude::*;
use runtime::prelude::*;
use tokio::io::AsyncWrite;
use tracing::{error, instrument};

#[tokio::main]
async fn main() -> Result<()> {
    runtime::setup()?;

    Node::builder()
        // broadcast, read, and topology are the three requests used in the maelstrom broadcast
        // workload.
        .handle("broadcast", broadcast)
        .handle("read", read)
        .handle("topology", topology)
        // replicate is an internal rpc used to propagate values to other nodes.
        .handle("replicate", replicate)
        .handle("replicate_ok", replicate_ok)
        .handle("error", error)
        .with_state(State::default())
        .run(tokio::io::stdin(), tokio::io::stdout())
        .await?;

    Ok(())
}

#[instrument(skip(rpc, ids, state))]
pub async fn broadcast<O>(rpc: Rpc<O>, ids: Ids, state: State, req: Broadcast) -> BroadcastOk
where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
{
    state.messages.write().await.push(req.message);
    replicate_message(rpc, ids, state, req.message);
    req.ok()
}

#[instrument(skip(state))]
pub async fn read(state: State, req: Read) -> ReadOk {
    req.ok(state.messages.read().await.clone())
}

#[instrument(skip(state))]
pub async fn topology(state: State, req: Topology) -> TopologyOk {
    *state.topology.write().await = req.topology;
    TopologyOk {
        headers: req.headers.reply(),
    }
}

fn replicate_message<O>(rpc: Rpc<O>, ids: Ids, _state: State, message: u64)
where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
{
    for node in ids.ids {
        tokio::spawn(replicate_message_to_node(
            rpc.clone(),
            Message {
                src: ids.id.clone(),
                dest: node.clone(),
                body: Replicate {
                    msg_id: MsgId(0),
                    message,
                },
            },
        ));
    }
}

async fn replicate_message_to_node<O>(rpc: Rpc<O>, msg: Message<Replicate>)
where
    O: AsyncWrite + Send + Sync + Unpin + 'static,
{
    rpc.send(&msg)
        .await
        .tap_err(|err| {
            error!(?err, ?msg, "failed to send replication rpc");
        })
        .expect("TODO: retry if error (maybe after some timeout?)");
    // TODO:
    // - store sent (or to-be-sent?) messages in state.pending
    // - wait to see if the msg is ack'd and resend after some timeout
}

#[instrument(skip(state))]
pub async fn replicate(state: State, req: Replicate) -> ReplicateOk {
    state.messages.write().await.push(req.message);
    req.ok()
}

#[instrument(skip(state))]
pub async fn replicate_ok(state: State, req: ReplicateOk) {
    if state
        .pending
        .write()
        .await
        .remove(&req.in_reply_to)
        .is_none()
    {
        error!(
            ?req,
            "Received ack for a Replicate message id that was never sent or already ack'd"
        );
    }
}

#[instrument(skip(state))]
pub async fn error(state: State, err: Error) {
    let source_id = match err.headers.in_reply_to {
        Some(id) => id,
        None => {
            error!(?err, "received error with no `in_reply_to` field");
            return;
        }
    };

    match state.pending.read().await.get(&source_id) {
        Some(req) => error!(?err, ?req, "received error in response to pending request"),
        None => error!(
            ?err,
            "received error in response to message never sent or no longer pending"
        ),
    }
}

#[derive(Clone, Default)]
pub struct State {
    messages: Arc<RwLock<Vec<u64>>>,
    topology: Arc<RwLock<HashMap<NodeId, Vec<NodeId>>>>,

    pending: Arc<RwLock<HashMap<MsgId, Message<Replicate>>>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "broadcast")]
pub struct Broadcast {
    #[serde(flatten)]
    headers: Headers,
    message: u64,
}

impl Broadcast {
    fn ok(self) -> BroadcastOk {
        BroadcastOk {
            headers: self.headers.reply(),
        }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(tag = "type", rename = "broadcast_ok")]
pub struct BroadcastOk {
    #[serde(flatten)]
    headers: Headers,
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "read")]
pub struct Read {
    #[serde(flatten)]
    headers: Headers,
}

impl Read {
    fn ok(self, messages: Vec<u64>) -> ReadOk {
        ReadOk {
            headers: self.headers.reply(),
            messages,
        }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(tag = "type", rename = "read_ok")]
pub struct ReadOk {
    #[serde(flatten)]
    headers: Headers,
    messages: Vec<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "topology")]
pub struct Topology {
    #[serde(flatten)]
    headers: Headers,
    topology: HashMap<NodeId, Vec<NodeId>>,
}

#[derive(Debug, Default, Serialize)]
#[serde(tag = "type", rename = "topology_ok")]
pub struct TopologyOk {
    #[serde(flatten)]
    headers: Headers,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename = "replicate")]
pub struct Replicate {
    msg_id: MsgId,
    message: u64,
}

impl Replicate {
    fn ok(self) -> ReplicateOk {
        ReplicateOk {
            in_reply_to: self.msg_id,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename = "replicate_ok")]
pub struct ReplicateOk {
    in_reply_to: MsgId,
}
