use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot::Sender;
use tokio::sync::Mutex;

use crate::message::{Headers, MsgId, NodeId};

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename = "init")]
pub struct Init {
    #[serde(flatten)]
    pub headers: Headers,
    pub node_id: NodeId,
    pub node_ids: Vec<NodeId>,
}

impl Init {
    pub fn ok(msg_id: Option<MsgId>) -> InitOk {
        InitOk {
            headers: Headers {
                in_reply_to: msg_id,
                ..Headers::default()
            },
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename = "init_ok")]
pub struct InitOk {
    #[serde(flatten)]
    pub headers: Headers,
}

#[derive(Clone, Debug, Default)]
pub struct Ids {
    pub(crate) id: NodeId,
    pub(crate) ids: Vec<NodeId>,
}

pub async fn init(ids: Arc<Mutex<Option<Sender<Ids>>>>, req: Init) -> InitOk {
    let ids = ids.lock().await.take().expect("already initialized");
    ids.send(Ids {
        id: req.node_id,
        ids: req.node_ids,
    })
    .expect("failed to initialize node; cannot proceed");
    Init::ok(req.headers.msg_id)
}
