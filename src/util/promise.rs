use std::{
    fmt::Debug,
    pin::Pin,
    task::{Context, Poll, Waker},
};

/// An easy to use wrapper around a future
///
/// The promise does not really do anything more than the future does.
/// It gets polled every time it is looked at, until it eventually yields an output.
/// However, it does hide away all of the complicated gears of the
/// Rust futures and provide an intuitive interface.
pub enum Promise<T> {
    Ready(T),

    Future(Pin<Box<dyn Future<Output = T>>>),
}

impl<T> Promise<T> {
    pub fn make(f: impl Future<Output = T> + 'static) -> Self {
        let mut promise = Promise::Future(Box::pin(f));
        promise.poll();

        promise
    }

    pub fn poll(&mut self) {
        let Self::Future(f) = self else { return };

        let mut cx = Context::from_waker(Waker::noop());
        let poll = f.as_mut().poll(&mut cx);

        if let Poll::Ready(o) = poll {
            *self = Self::Ready(o);
        }
    }

    pub fn get(&mut self) -> Option<&T> {
        self.poll();

        match self {
            Self::Ready(o) => Some(o),
            Self::Future(_) => None,
        }
    }
}

impl<T: Debug> Debug for Promise<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready(o) => {
                write!(f, "[Ready Promise : {o:?}]")
            }
            Self::Future(_) => {
                write!(f, "[Waiting Promise]")
            }
        }
    }
}
