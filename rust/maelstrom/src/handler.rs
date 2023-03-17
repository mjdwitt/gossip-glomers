use std::error::Error;
use std::future::Future;
use std::pin::Pin;

use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use tailsome::*;

pub trait Handler {
    fn call(self, raw: String) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn Error>>>>>;
}

impl<Fut, Req, Res> Handler for Box<dyn HandlerFn<Fut, Req, Res>>
where
    Fut: Future<Output = Res> + Send + 'static,
    Req: DeserializeOwned + Send + 'static,
    Res: Serialize + Send + 'static,
{
    fn call(self, raw: String) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn Error>>>>> {
        Box::pin(async move {
            let req: Req = serde_json::from_str(&raw)?;
            let res: Res = self.callf(req).await;
            serde_json::to_string(&res)?.into_ok()
        })
    }
}

pub trait HandlerFn<Fut, Req, Res> {
    fn callf(&self, req: Req) -> Fut;
}

impl<F, Fut, Req, Res> HandlerFn<Fut, Req, Res> for F
where
    F: Fn(Req) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send,
    Req: DeserializeOwned + Send + 'static,
    Res: Serialize + Send + 'static,
{
    fn callf(&self, req: Req) -> Fut {
        self(req)
    }
}

#[cfg(test)]
pub mod test {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Deserialize)]
    struct Request(u32);

    #[derive(Serialize)]
    struct Response(String);

    async fn _test(req: Request) -> Response {
        Response(req.0.to_string())
    }

    fn _test_is_handler() {
        receives_handler(Box::new(_test));
    }

    pub fn receives_handler<Fut, Req, Res>(f: impl HandlerFn<Fut, Req, Res> + 'static)
    where
        Fut: Future<Output = Res> + Send + 'static,
        Req: DeserializeOwned + Send + 'static,
        Res: Serialize + Send + 'static,
    {
        let f: Box<dyn HandlerFn<Fut, Req, Res>> = Box::new(f);
        let _: Box<dyn Handler> = Box::new(f);
    }
}
