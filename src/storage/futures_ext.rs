use futures::sync::oneshot::spawn;
use futures::Future;
use lazy_static::lazy_static;
use tokio::runtime::Runtime;

lazy_static! {
    static ref RUNTIME: Runtime = Runtime::new().unwrap();
}

pub trait FuturesExt: Future
where
    Self: Sized + Send + 'static,
    Self::Item: Send,
    Self::Error: Send,
{
    fn sync(self) -> Result<Self::Item, Self::Error> {
        spawn(self, &RUNTIME.executor()).wait()
    }
}

impl<F> FuturesExt for F
where
    F: Future + Send + 'static,
    F::Item: Send,
    F::Error: Send,
{
}
