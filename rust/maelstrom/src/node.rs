#![allow(dead_code)] // TODO: remove this

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use tokio::io::{AsyncBufRead, AsyncBufReadExt};
use tokio::sync::RwLock;

use crate::handler::Handler;
use crate::message::NodeId;

pub mod init;

use init::{Init, InitOk};

#[derive(Clone, Default)]
pub struct Node {
    ids: Arc<RwLock<IdState>>,
    handlers: Arc<RwLock<HashMap<&'static str, Pin<Box<dyn Handler>>>>>,
}

#[derive(Default)]
struct IdState {
    id: NodeId,
    node_ids: Vec<NodeId>,
}

impl IdState {
    fn init(&mut self, req: Init) -> InitOk {
        self.id = req.node_id.clone().into();
        self.node_ids = req
            .node_ids
            .clone()
            .into_iter()
            .map(|id| id.into())
            .collect();
        req.ok()
    }
}

impl Node {
    pub fn new() -> Self {
        let n = Self::default();

        // n.handle("init", Box::new(init::init));

        n
    }

    async fn init(&self, req: Init) -> InitOk {
        self.ids.write().await.init(req)
    }

    pub fn handle<Q, S, E>(&mut self, _type_: &'static str, _handler: impl Handler) -> &mut Self {
        self
    }

    pub async fn run<I, O>(self, i: I)
    where
        I: AsyncBufRead + Send + Sync + Unpin,
    {
        let mut lines = i.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tokio::spawn(run_request(line));
        }
    }
}

async fn run_request(_raw: String) {
    todo!()
}
