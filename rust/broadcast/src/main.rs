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
pub async fn broadcast(state: State<()>, req: Broadcast) -> BroadcastOk {
    todo!()
}

#[instrument(skip(state))]
pub async fn read(state: State<()>, req: Read) -> ReadOk {
    todo!()
}

#[instrument(skip(state))]
pub async fn topology(state: State<()>, req: Topology) -> TopologyOk {
    todo!()
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "broadcast")]
pub struct Broadcast {
    #[serde(flatten)]
    headers: Headers,
    message: u64,
}

impl Body for Broadcast {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(tag = "type", rename = "broadcast_ok")]
pub struct BroadcastOk {
    #[serde(flatten)]
    headers: Headers,
}

impl Body for BroadcastOk {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "read")]
pub struct Read {
    #[serde(flatten)]
    headers: Headers,
}

impl Body for Read {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(tag = "type", rename = "read_ok")]
pub struct ReadOk {
    #[serde(flatten)]
    headers: Headers,
    messages: Vec<u64>,
}

impl Body for ReadOk {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "topology")]
pub struct Topology {
    #[serde(flatten)]
    headers: Headers,
    topology: HashMap<NodeId, Vec<NodeId>>,
}

impl Body for Topology {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(tag = "type", rename = "topology_ok")]
pub struct TopologyOk {
    #[serde(flatten)]
    headers: Headers,
}

impl Body for TopologyOk {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}
