use std::sync::Arc;

use futures::future::Shared;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot::{Receiver, Sender};
use tokio::sync::Mutex;

use crate::message::NodeId;

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename = "init")]
pub struct Init {
    pub node_id: NodeId,
    pub node_ids: Vec<NodeId>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename = "init_ok")]
pub struct InitOk {}

#[derive(Clone, Debug, Default)]
pub struct Ids {
    pub id: NodeId,
    pub ids: Vec<NodeId>,
}

pub async fn init(ids: IdTx, req: Init) -> InitOk {
    let ids = ids.lock().await.take().expect("already initialized");
    ids.send(Ids {
        id: req.node_id,
        ids: req.node_ids,
    })
    .expect("failed to initialize node; cannot proceed");
    InitOk {}
}

pub type IdTx = Arc<Mutex<Option<Sender<Ids>>>>;
pub type IdRx = Shared<Receiver<Ids>>;

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn serialize_init_ok() {
        assert_eq!(
            r#"{"type":"init_ok"}"#,
            serde_json::to_string(&InitOk {}).unwrap(),
        );
    }
}
