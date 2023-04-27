use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tailsome::*;

use crate::message::{Message, Request, Response};
use crate::node::init::Ids;
use crate::node::rpc;
use crate::node::state::{FromRef, State};

pub type RawResponse = Result<serde_json::Value, Box<dyn Error>>;

pub trait ErasedHandler<Rpc, S>: Send + Sync
where
    Rpc: Clone,
    S: Clone + FromRef<State<S>>,
{
    fn call(
        self: Arc<Self>,
        rpc: Rpc,
        ids: Ids,
        state: S,
        raw_request: String,
    ) -> Pin<Box<dyn Future<Output = RawResponse> + Send>>;
}

impl<T, S, Rpc, Req, Res, Fut> ErasedHandler<Rpc, S> for Box<dyn Handler<T, S, Rpc, Req, Res, Fut>>
where
    T: 'static,
    S: FromRef<State<S>> + Clone + Send + Sync + 'static,
    Rpc: Clone + Send + Sync + 'static,
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send + 'static,
{
    fn call(
        self: Arc<Self>,
        rpc: Rpc,
        ids: Ids,
        state: S,
        raw_request: String,
    ) -> Pin<Box<dyn Future<Output = RawResponse> + Send>> {
        let h = self.clone();
        Box::pin(async move {
            let req: Message<Req> = serde_json::from_str(&raw_request)?;
            let res = h.callf(rpc, ids, state, req.body.body).await;
            serde_json::to_value(&res)?.into_ok()
        })
    }
}

pub trait Handler<T, S, Rpc, Req, Res, Fut>: Send + Sync
where
    Fut: Future<Output = Res>,
{
    fn callf(&self, rpc: Rpc, ids: Ids, state: S, req: Req) -> Fut;
}

impl<S, Rpc, Req, Res, Fut, F> Handler<(Rpc, Ids, S), S, Rpc, Req, Res, Fut> for F
where
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    Rpc: rpc::Rpc,
    F: Fn(Rpc, Ids, S, Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, rpc: Rpc, ids: Ids, state: S, req: Req) -> Fut {
        self(rpc, ids, state, req)
    }
}

impl<S, Rpc, Req, Res, Fut, F> Handler<(Ids, S), S, Rpc, Req, Res, Fut> for F
where
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(Ids, S, Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, _: Rpc, ids: Ids, state: S, req: Req) -> Fut {
        self(ids, state, req)
    }
}

impl<S, Rpc, Req, Res, Fut, F> Handler<(S,), S, Rpc, Req, Res, Fut> for F
where
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(S, Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, _: Rpc, _: Ids, state: S, req: Req) -> Fut {
        self(state, req)
    }
}

impl<S, Rpc, Req, Res, Fut, F> Handler<(), S, Rpc, Req, Res, Fut> for F
where
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, _: Rpc, _: Ids, _: S, req: Req) -> Fut {
        self(req)
    }
}
