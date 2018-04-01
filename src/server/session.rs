use std::io;
use std::io::{Read, Write};
use std::iter::repeat;
use std::net::SocketAddr;

use futures::Future;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;

/// The result of a non-blocking operation.
pub enum NonBlocking<T> {
    Success(T),
    Err(io::Error),
    WouldBlock
}

/// A remote connection for a user.
pub struct Session {
    pub id: String,
    pub stream: TcpStream
}

impl Session {
    /// Establish a new session.
    pub fn connect(
        id: String,
        addr: &SocketAddr,
        handle: &Handle
    ) -> Box<Future<Item = Session, Error = io::Error>> {
        Box::new(TcpStream::connect(addr, handle).map(|stream| {
            Session{id: id, stream: stream}
        }))
    }

    /// Read a chunk of data from the session.
    ///
    /// Yields an empty chunk on EOF.
    pub fn read_chunk(&mut self, max_size: usize) -> NonBlocking<Vec<u8>> {
        let mut buffer: Vec<u8> = repeat(0).take(max_size).collect();
        match self.stream.read(&mut buffer) {
            Ok(size) => NonBlocking::Success(buffer[..size].to_vec()),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    NonBlocking::WouldBlock
                } else {
                    NonBlocking::Err(e)
                }
            }
        }
    }

    /// Write a chunk of data to the session.
    ///
    /// May not write all (or any) of the data.
    /// If 0 bytes are written, it likely indicates an error.
    pub fn write_chunk(&mut self, chunk: &[u8]) -> NonBlocking<usize> {
        match self.stream.write(chunk) {
            Ok(size) => NonBlocking::Success(size),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    NonBlocking::WouldBlock
                } else {
                    NonBlocking::Err(e)
                }
            }
        }
    }
}
