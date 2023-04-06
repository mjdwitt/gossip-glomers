use super::init::IdRx;

#[derive(Clone)]
pub struct State<S: Clone> {
    pub(crate) ids: IdRx,
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

impl<S: Clone> FromRef<State<S>> for S {
    fn from_ref(input: &State<S>) -> S {
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
