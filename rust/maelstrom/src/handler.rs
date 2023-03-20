use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tailsome::*;
use tokio::sync::RwLock;
use tracing::*;

use crate::message::{Message, Request, Response};

pub type State<S> = Arc<RwLock<S>>;

pub trait ErasedHandler<S>: Send + Sync {
    fn call(
        self: Arc<Self>,
        state: State<S>,
        raw_request: String,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Box<dyn Error>>> + Send>>;
}

impl<S, Fut, Req, Res> ErasedHandler<S> for Box<dyn Handler<Fut, Req, Res, S>>
where
    S: Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Req: Request + 'static,
    Res: Response + 'static,
{
    fn call(
        self: Arc<Self>,
        state: State<S>,
        raw_request: String,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Box<dyn Error>>> + Send>> {
        let h = self.clone();
        Box::pin(async move {
            let req: Message<Req> = serde_json::from_str(&raw_request)?;
            debug!(?req, "parsed request");
            let res: Res = h.callf(state, req.body).await;
            debug!(?res, "built response");
            serde_json::to_vec(&res)?.into_ok()
        })
    }
}

pub trait Handler<Fut, Req, Res, S>: Send + Sync {
    fn callf(&self, state: State<S>, req: Req) -> Fut;
}

impl<F, Fut, Req, Res, S> Handler<Fut, Req, Res, S> for F
where
    F: Fn(State<S>, Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send,
    Req: Request + 'static,
    Res: Response + 'static,
{
    fn callf(&self, state: State<S>, req: Req) -> Fut {
        self(state, req)
    }
}

#[cfg(test)]
pub mod test {
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::message::{Body, Headers};

    #[derive(Debug, Deserialize)]
    struct Test(u32);

    impl Body for Test {
        fn headers(&self) -> &Headers {
            todo!()
        }
    }

    #[derive(Debug, Serialize)]
    struct TestOk(String);

    impl Body for TestOk {
        fn headers(&self) -> &Headers {
            todo!()
        }
    }

    async fn _test(_: State<()>, req: Test) -> TestOk {
        TestOk(req.0.to_string())
    }

    fn _test_is_handler() {
        receives_handler(Box::new(_test));
    }

    pub fn receives_handler<Fut, Req, Res, S>(f: impl Handler<Fut, Req, Res, S> + 'static)
    where
        S: Send + Sync + 'static,
        Fut: Future<Output = Res> + Send + 'static,
        Req: Request + 'static,
        Res: Response + 'static,
    {
        let f: Box<dyn Handler<Fut, Req, Res, S>> = Box::new(f);
        let _: Box<dyn ErasedHandler<S>> = Box::new(f);
    }
}
