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
        .with_state(State::default())
        .run(tokio::io::stdin(), tokio::io::stdout())
        .await?;

    Ok(())
}

#[derive(Clone, Default)]
pub struct State {
    messages: Arc<RwLock<Vec<u64>>>,
    topology: Arc<RwLock<HashMap<NodeId, Vec<NodeId>>>>,
}

//  _                    _
// | |_ ___  _ __   ___ | | ___   __ _ _   _
// | __/ _ \| '_ \ / _ \| |/ _ \ / _` | | | |
// | || (_) | |_) | (_) | | (_) | (_| | |_| |
//  \__\___/| .__/ \___/|_|\___/ \__, |\__, |
//          |_|                  |___/ |___/

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "topology")]
pub struct Topology {
    topology: HashMap<NodeId, Vec<NodeId>>,
}

#[derive(Debug, Default, Serialize)]
#[serde(tag = "type", rename = "topology_ok")]
pub struct TopologyOk {}

#[instrument(skip(state))]
pub async fn topology(state: State, req: Topology) -> TopologyOk {
    *state.topology.write().await = req.topology;
    TopologyOk {}
}

//                     _
//  _ __ ___  __ _  __| |
// | '__/ _ \/ _` |/ _` |
// | | |  __/ (_| | (_| |
// |_|  \___|\__,_|\__,_|

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "read")]
pub struct Read {}

impl Read {
    fn ok(self, messages: Vec<u64>) -> ReadOk {
        ReadOk { messages }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(tag = "type", rename = "read_ok")]
pub struct ReadOk {
    messages: Vec<u64>,
}

#[instrument(skip(state))]
pub async fn read(state: State, req: Read) -> ReadOk {
    req.ok(state.messages.read().await.clone())
}

//  _                         _               _
// | |__  _ __ ___   __ _  __| | ___ __ _ ___| |_
// | '_ \| '__/ _ \ / _` |/ _` |/ __/ _` / __| __|
// | |_) | | | (_) | (_| | (_| | (_| (_| \__ \ |_
// |_.__/|_|  \___/ \__,_|\__,_|\___\__,_|___/\__|

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "broadcast")]
pub struct Broadcast {
    message: u64,
}

#[derive(Debug, Default, Serialize)]
#[serde(tag = "type", rename = "broadcast_ok")]
pub struct BroadcastOk {}

#[instrument(skip(rpc, ids, state))]
pub async fn broadcast(rpc: impl Rpc, ids: Ids, state: State, req: Broadcast) -> BroadcastOk {
    state.messages.write().await.push(req.message);
    replicate_message(rpc, ids, req.message).await;
    BroadcastOk {}
}

//                 _ _           _
//  _ __ ___ _ __ | (_) ___ __ _| |_ ___
// | '__/ _ \ '_ \| | |/ __/ _` | __/ _ \
// | | |  __/ |_) | | | (_| (_| | ||  __/
// |_|  \___| .__/|_|_|\___\__,_|\__\___|
//          |_|

async fn replicate_message(rpc: impl Rpc, ids: Ids, message: u64) {
    for dest in ids.ids {
        if dest != ids.id {
            rpc.send_rpc(ids.id.clone(), dest, &Replicate { message })
                .await
                .unwrap();
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename = "replicate")]
pub struct Replicate {
    message: u64,
}

impl Replicate {
    fn ok(self) -> ReplicateOk {
        ReplicateOk {}
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename = "replicate_ok")]
pub struct ReplicateOk {}

#[instrument(skip(state))]
pub async fn replicate(state: State, req: Replicate) -> ReplicateOk {
    state.messages.write().await.push(req.message);
    req.ok()
}

#[instrument]
pub async fn replicate_ok(req: ReplicateOk) {}
