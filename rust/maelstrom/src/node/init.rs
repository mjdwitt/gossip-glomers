use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

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

#[derive(Debug, Default)]
pub struct Ids {
    pub id: NodeId,
    pub ids: Vec<NodeId>,
}

pub async fn init(ids: Arc<RwLock<Ids>>, req: Init) -> InitOk {
    let mut ids = ids.write().await;
    ids.id = req.node_id;
    ids.ids = req.node_ids;
    Init::ok(req.headers.msg_id)
}

#[cfg(test)]
fn _init_is_handler() {
    crate::handler::test::receives_handler(init);
}
