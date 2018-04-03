use std::io;
use std::iter::repeat;

use futures::{Async, Future, IntoFuture, Poll, Stream};
use tokio_io::AsyncRead;

/// Get a Future for when the stream completes.
pub fn wait_for_end<S: Stream + 'static>(s: S) -> Box<Future<Item = (), Error = S::Error>> {
    Box::new(s.for_each(|_| Ok(()).into_future()))
}

pub struct ReadStream<T: AsyncRead> {
    reader: T,
    buf_size: usize
}

impl<T: AsyncRead> ReadStream<T> {
    pub fn new(reader: T, buf_size: usize) -> ReadStream<T> {
        ReadStream{reader: reader, buf_size: buf_size}
    }
}

impl<T: AsyncRead> Stream for ReadStream<T> {
    type Item = Vec<u8>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let mut res: Vec<u8> = repeat(0).take(self.buf_size).collect();
        match self.reader.poll_read(&mut res) {
            Ok(Async::Ready(size)) => {
                if size == 0 {
                    Ok(Async::Ready(None))
                } else {
                    Ok(Async::Ready(Some(res[..size].to_vec())))
                }
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e)
        }
    }
}
