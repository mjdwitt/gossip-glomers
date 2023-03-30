use std::collections::HashMap;

use maelstrom::prelude::*;
use runtime::prelude::*;
use tracing::instrument;

#[tokio::main]
async fn main() -> Result<()> {
    runtime::setup()?;

    Node::builder()
        .handle("broadcast", broadcast)
        .handle("read", read)
        .handle("topology", topology)
        .with_state(Default::default())
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

