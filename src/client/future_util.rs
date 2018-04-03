use futures::{Future, IntoFuture, Stream};
use tokio_io::{AsyncRead, AsyncWrite};

/// Get a Future for when the stream completes.
pub fn wait_for_end<S: Stream + 'static>(s: S) -> Box<Future<Item = (), Error = S::Error>> {
    Box::new(s.for_each(|_| Ok(()).into_future()))
}
