use rpc_toolkit::reqwest::Client;
use rpc_toolkit::url::{Host, Url};
use rpc_toolkit::{Context, SeedableContext};

#[derive(Debug, Clone)]
pub struct ExtendedContext<T, U> {
    base: T,
    pub extension: U,
}
impl<T, U> ExtendedContext<T, U> {
    pub fn map<F: FnOnce(U) -> V, V>(self, f: F) -> ExtendedContext<T, V> {
        ExtendedContext {
            base: self.base,
            extension: f(self.extension),
        }
    }
    pub fn base(&self) -> &T {
        &self.base
    }
}
impl<T> From<T> for ExtendedContext<T, ()> {
    fn from(base: T) -> Self {
        ExtendedContext {
            base,
            extension: (),
        }
    }
}
impl<T: Clone + Context> SeedableContext<T> for ExtendedContext<T, ()> {
    fn new(seed: T) -> Self {
        seed.into()
    }
}
impl<T: Context, U> Context for ExtendedContext<T, U> {
    fn host(&self) -> Host<&str> {
        self.base.host()
    }
    fn port(&self) -> u16 {
        self.base.port()
    }
    fn protocol(&self) -> &str {
        self.base.protocol()
    }
    fn url(&self) -> Url {
        self.base.url()
    }
    fn client(&self) -> &Client {
        self.base.client()
    }
}

pub enum EitherContext<A: Context, B: Context> {
    A(A),
    B(B),
}
impl<A: Context, B: Context> Context for EitherContext<A, B> {
    fn host(&self) -> Host<&str> {
        match self {
            EitherContext::A(a) => a.host(),
            EitherContext::B(b) => b.host(),
        }
    }
    fn port(&self) -> u16 {
        match self {
            EitherContext::A(a) => a.port(),
            EitherContext::B(b) => b.port(),
        }
    }
    fn protocol(&self) -> &str {
        match self {
            EitherContext::A(a) => a.protocol(),
            EitherContext::B(b) => b.protocol(),
        }
    }
    fn url(&self) -> Url {
        match self {
            EitherContext::A(a) => a.url(),
            EitherContext::B(b) => b.url(),
        }
    }
    fn client(&self) -> &Client {
        match self {
            EitherContext::A(a) => a.client(),
            EitherContext::B(b) => b.client(),
        }
    }
}
