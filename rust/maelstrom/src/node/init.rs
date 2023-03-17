use serde::{Deserialize, Serialize};

use crate::message::{Body, Headers, NodeId};

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
    pub fn ok(self) -> InitOk {
        InitOk {
            headers: Headers {
                type_: "init_ok".into(),
                in_reply_to: self.headers.msg_id,
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

pub async fn init(req: Init) -> InitOk {
    req.ok()
}

#[cfg(test)]
fn init_is_handler() {
    crate::handler::test::receives_handler(init);
}
