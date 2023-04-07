use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tailsome::*;
use tokio::io::AsyncWrite;
use tracing::*;

use crate::message::{Message, Request, Response};
use crate::node::init::Ids;
use crate::node::rpc::Rpc;
use crate::node::state::{FromRef, State};

pub type RawResponse = Result<Vec<u8>, Box<dyn Error>>;

pub trait ErasedHandler<O, S>: Send + Sync
where
    O: AsyncWrite + Unpin,
    S: Clone + FromRef<State<S>>,
{
    fn call(
        self: Arc<Self>,
        rpc: Rpc<O>,
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
        rpc: Rpc<O>,
        ids: Ids,
        state: S,
        raw_request: String,
    ) -> Pin<Box<dyn Future<Output = RawResponse> + Send>> {
        let h = self.clone();
        Box::pin(async move {
            let req: Message<Req> = serde_json::from_str(&raw_request)?;
            debug!(?req, "parsed request");
            let res = Message {
                src: req.dest,
                dest: req.src,
                body: h.callf(rpc, ids, state, req.body).await,
            };
            debug!(?res, "built response");
            serde_json::to_vec(&res)?.into_ok()
        })
    }
}

pub trait Handler<T, S, O, Req, Res, Fut>: Send + Sync
where
    O: AsyncWrite + Unpin,
    Fut: Future<Output = Res>,
{
    fn callf(&self, rpc: Rpc<O>, ids: Ids, state: S, req: Req) -> Fut;
}

impl<S, O, Req, Res, Fut, F> Handler<(Rpc<O>, Ids, S), S, O, Req, Res, Fut> for F
where
    O: AsyncWrite + Unpin,
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(Rpc<O>, Ids, S, Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, rpc: Rpc<O>, ids: Ids, state: S, req: Req) -> Fut {
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
    fn callf(&self, _: Rpc<O>, ids: Ids, state: S, req: Req) -> Fut {
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
    fn callf(&self, _: Rpc<O>, _: Ids, state: S, req: Req) -> Fut {
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
    fn callf(&self, _: Rpc<O>, _: Ids, _: S, req: Req) -> Fut {
        self(req)
    }
}
