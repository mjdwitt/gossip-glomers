use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tailsome::*;
use tokio::sync::RwLock;
use tracing::*;

use crate::message::{Message, Request, Response};
use crate::node::state::{FromRef, NodeState};

pub type State<S> = Arc<RwLock<S>>;
pub type RawResponse = Result<Vec<u8>, Box<dyn Error>>;

pub trait ErasedHandler<S: Clone + FromRef<NodeState<S>>>: Send + Sync {
    fn call(
        self: Arc<Self>,
        state: S,
        raw_request: String,
    ) -> Pin<Box<dyn Future<Output = RawResponse> + Send>>;
}

impl<T, S, Req, Res, Fut> ErasedHandler<S> for Box<dyn Handler<T, S, Req, Res, Fut>>
where
    T: 'static,
    S: FromRef<NodeState<S>> + Clone + Send + Sync + 'static,
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send + 'static,
{
    fn call(
        self: Arc<Self>,
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
                body: h.callf(state, req.body).await,
            };
            debug!(?res, "built response");
            serde_json::to_vec(&res)?.into_ok()
        })
    }
}

pub trait Handler<T, S, Req, Res, Fut: Future<Output = Res>>: Send + Sync {
    fn callf(&self, state: S, req: Req) -> Fut;
}

impl<S, Req, Res, Fut, F> Handler<(S,), S, Req, Res, Fut> for F
where
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(S, Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, state: S, req: Req) -> Fut {
        self(state, req)
    }
}

impl<S, Req, Res, Fut, F> Handler<(), S, Req, Res, Fut> for F
where
    Req: Request + 'static,
    Res: Response + 'static,
    Fut: Future<Output = Res> + Send,
    F: Fn(Req) -> Fut + Clone + Send + Sync + 'static,
{
    fn callf(&self, _: S, req: Req) -> Fut {
        self(req)
    }
}

#[cfg(test)]
pub mod test {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Deserialize)]
    struct Test(u32);

    #[derive(Debug, Serialize)]
    struct TestOk(String);

    async fn _test(_: State<()>, req: Test) -> TestOk {
        TestOk(req.0.to_string())
    }

    fn _test_is_handler() {
        receives_handler(Box::new(_test));
    }

    pub fn receives_handler<T, S, Req, Res, Fut>(f: impl Handler<T, S, Req, Res, Fut> + 'static)
    where
        T: 'static,
        S: FromRef<NodeState<S>> + Clone + Send + Sync + 'static,
        Req: Request + 'static,
        Res: Response + 'static,
        Fut: Future<Output = Res> + Send + 'static,
    {
        let f: Box<dyn Handler<T, S, Req, Res, Fut>> = Box::new(f);
        let _: Box<dyn ErasedHandler<S>> = Box::new(f);
    }
}
