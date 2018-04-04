use std::io;
use std::iter::repeat;
use std::mem::replace;

use futures::{Async, AsyncSink, Future, IntoFuture, Poll, Sink, StartSend, Stream};
use tokio_io::{AsyncRead, AsyncWrite};

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

pub struct WriteSink<T: AsyncWrite> {
    writer: T,
    cur_buf: Option<Vec<u8>>
}

impl<T: AsyncWrite> WriteSink<T> {
    pub fn new(writer: T) -> WriteSink<T> {
        WriteSink{writer: writer, cur_buf: None}
    }

    fn attempt_write(&mut self) -> Poll<(), io::Error> {
        while self.cur_buf.is_some() {
            let buf = replace(&mut self.cur_buf, None).unwrap();
            self.cur_buf = match self.writer.poll_write(&buf)? {
                Async::Ready(size) => {
                    if size == buf.len() {
                        None
                    } else {
                        Some(buf[size..buf.len()].to_vec())
                    }
                },
                Async::NotReady => Some(buf)
            };
        }
        Ok(Async::Ready(()))
    }
}

impl<T: AsyncWrite> Sink for WriteSink<T> {
    type SinkItem = Vec<u8>;
    type SinkError = io::Error;

    fn start_send(
        &mut self,
        item: Self::SinkItem
    ) -> StartSend<Self::SinkItem, Self::SinkError> {
        Ok(match self.attempt_write()? {
            Async::Ready(_) => {
                self.cur_buf = Some(item);
                AsyncSink::Ready
            }
            Async::NotReady => AsyncSink::NotReady(item)
        })
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        match self.attempt_write()? {
            Async::Ready(_) => self.writer.poll_flush(),
            x => Ok(x)
        }
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        match self.poll_complete()? {
            Async::Ready(_) => self.writer.shutdown(),
            x => Ok(x)
        }
    }
}
