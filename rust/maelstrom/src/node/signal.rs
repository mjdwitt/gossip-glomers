use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::future::FutureExt;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tokio::time::Sleep;

pub trait Signal {
    type Output: Future<Output = ()> + Send;
    fn signal(&self) -> Self::Output;
}

impl Signal for Arc<Mutex<Receiver<()>>> {
    type Output = Pin<Box<dyn Future<Output = ()> + Send>>;

    fn signal(&self) -> Self::Output {
        let self_ = self.clone();
        async move { self_.lock().await.recv().await.unwrap() }.boxed()
    }
}

#[derive(Clone)]
pub struct TimedSignal {
    duration: Duration,
}

impl TimedSignal {
    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

impl Signal for TimedSignal {
    type Output = Sleep;

    fn signal(&self) -> Self::Output {
        tokio::time::sleep(self.duration)
    }
}

#[derive(Clone)]
pub struct Never;

impl Signal for Never {
    type Output = futures::future::Pending<()>;

    fn signal(&self) -> Self::Output {
        futures::future::pending()
    }
}
