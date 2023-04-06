use std::collections::HashMap;

use maelstrom::prelude::*;
use runtime::prelude::*;
use tracing::instrument;

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
        .with_state(Arc::new(RwLock::new(Data::default())))
        .run(tokio::io::stdin(), tokio::io::stdout())
        .await?;

    Ok(())
}

#[instrument(skip(state))]
pub async fn broadcast(state: State, req: Broadcast) -> BroadcastOk {
    state.write().await.messages.push(req.message);
    req.ok()
}

#[instrument(skip(state))]
pub async fn read(state: State, req: Read) -> ReadOk {
    req.ok(state.read().await.messages.clone())
}

#[instrument(skip(state))]
pub async fn topology(state: State, req: Topology) -> TopologyOk {
    state.write().await.topology = req.topology;
    TopologyOk {
        headers: req.headers.reply(),
    }
}

#[instrument(skip(_state))]
pub async fn replicate(_state: State, req: Replicate) -> ReplicateOk {
    req.ok()
}

#[instrument(skip(_state))]
pub async fn replicate_ok(_state: State, _req: ReplicateOk) {
    todo!()
}

#[instrument(skip(_state))]
pub async fn error(_state: State, _req: Error) {
    todo!()
}

type State = Arc<RwLock<Data>>;

#[derive(Default)]
pub struct Data {
    messages: Vec<u64>,
    topology: HashMap<NodeId, Vec<NodeId>>,
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

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "replicate")]
pub struct Replicate {
    #[serde(flatten)]
    headers: Headers,
}

impl Replicate {
    fn ok(self) -> ReplicateOk {
        ReplicateOk {
            headers: self.headers.reply(),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(tag = "type", rename = "replicate_ok")]
pub struct ReplicateOk {
    #[serde(flatten)]
    headers: Headers,
}
