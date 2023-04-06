use futures::future::Shared;
use tokio::sync::oneshot::Receiver;

use super::init::Ids;

pub struct State;

#[derive(Clone)]
pub struct NodeState<S: Clone> {
    pub(crate) ids: Shared<Receiver<Ids>>,
    pub(crate) app: S,
}

pub trait FromRef<T> {
    fn from_ref(input: &T) -> Self;
}

impl<T: Clone> FromRef<T> for T {
    fn from_ref(input: &T) -> Self {
        input.clone()
    }
}

impl<S: Clone> FromRef<NodeState<S>> for S {
    fn from_ref(input: &NodeState<S>) -> S {
        input.app.clone()
    }
}

pub trait RefInner<T> {
    fn inner(&self) -> T;
}

impl<B, A: FromRef<B>> RefInner<A> for B {
    fn inner(&self) -> A {
        A::from_ref(self)
    }
}
