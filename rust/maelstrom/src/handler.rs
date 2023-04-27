use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tailsome::*;
use tokio::io::AsyncWrite;

use crate::message::{Message, Request, Response};
use crate::node::init::Ids;
use crate::node::rpc::RpcWriter;
use crate::node::state::{FromRef, State};

pub type RawResponse = Result<serde_json::Value, Box<dyn Error>>;

pub trait ErasedHandler<O, S>: Send + Sync
where
    O: AsyncWrite + Unpin,
    S: Clone + FromRef<State<S>>,
{
    fn call(
        self: Arc<Self>,
        rpc: RpcWriter<O>,
        ids: Ids,
        state: S,
        raw_request: String,
    ) -> Pin<Box<dyn Future<Output = RawResponse> + Send>>;
}

impl<T, S, O, Req, Res, Fut> ErasedHandler<O, S> for Box<dyn Handler<T, S, O, Req, Res, Fut>>
where
    T: 'static,
    S: FromRef<State<S>> + Clone + Send + Sync + 'static,
    O: AsyncWrite + Send + Sync + Unpin + 'static,
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send + 'static,
{
    fn call(
        self: Arc<Self>,
        rpc: RpcWriter<O>,
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

pub trait Handler<T, S, O, Req, Res, Fut>: Send + Sync
where
    O: AsyncWrite + Unpin,
    Fut: Future<Output = Res>,
{
    fn callf(&self, rpc: RpcWriter<O>, ids: Ids, state: S, req: Req) -> Fut;
}

impl<S, O, Req, Res, Fut, F> Handler<(RpcWriter<O>, Ids, S), S, O, Req, Res, Fut> for F
where
    O: AsyncWrite + Unpin,
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(RpcWriter<O>, Ids, S, Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, rpc: RpcWriter<O>, ids: Ids, state: S, req: Req) -> Fut {
        self(rpc, ids, state, req)
    }
}

impl<S, O, Req, Res, Fut, F> Handler<(Ids, S), S, O, Req, Res, Fut> for F
where
    O: AsyncWrite + Unpin,
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(Ids, S, Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, _: RpcWriter<O>, ids: Ids, state: S, req: Req) -> Fut {
        self(ids, state, req)
    }
}

impl<S, O, Req, Res, Fut, F> Handler<(S,), S, O, Req, Res, Fut> for F
where
    O: AsyncWrite + Unpin,
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(S, Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, _: RpcWriter<O>, _: Ids, state: S, req: Req) -> Fut {
        self(state, req)
    }
}

impl<S, O, Req, Res, Fut, F> Handler<(), S, O, Req, Res, Fut> for F
where
    O: AsyncWrite + Unpin,
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, _: RpcWriter<O>, _: Ids, _: S, req: Req) -> Fut {
        self(req)
    }
}
