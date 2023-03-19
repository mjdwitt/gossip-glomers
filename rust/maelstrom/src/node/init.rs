use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::message::{Body, Headers, MsgId, NodeId};

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Init {
    #[serde(flatten)]
    pub headers: Headers,
    pub node_id: NodeId,
    pub node_ids: Vec<NodeId>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct InitOk {
    #[serde(flatten)]
    pub headers: Headers,
}

impl Init {
    pub fn ok(msg_id: Option<MsgId>) -> InitOk {
        InitOk {
            headers: Headers {
                type_: "init_ok".into(),
                in_reply_to: msg_id,
                ..Headers::default()
            },
        }
    }
}

impl Body for Init {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}

#[derive(Default)]
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
