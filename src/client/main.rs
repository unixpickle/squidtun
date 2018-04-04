extern crate clap;
extern crate futures;
extern crate hyper;
extern crate squidtun;
extern crate tokio_core;
extern crate tokio_io;

#[macro_use]
extern crate log;
extern crate simple_logger;

mod future_util;

use std::net::SocketAddr;

use clap::{App, Arg};
use futures::{Future, Sink, Stream};
use futures::future::{Loop, join_all, loop_fn};
use futures::stream::repeat;
use hyper::{Method, Request, StatusCode};
use hyper::client::{Client, HttpConnector};
use hyper::header::{Connection, Host};
use log::Level;
use squidtun::{current_proof, generate_session_id};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;

use future_util::{ReadStream, WriteSink, wait_for_end};

const CONCURRENT_CONNS: usize = 5;
const MAX_READ_SIZE: usize = 65536;

#[derive(Clone, Debug)]
struct HostInfo {
    proxy_addr: SocketAddr,
    host: String,
    password: String
}

type SessionInfo = (Client<HttpConnector>, HostInfo, String);

fn main() {
    simple_logger::init_with_level(Level::Info).unwrap();

    let matches = App::new("squidtun-server")
        .arg(Arg::with_name("password")
            .short("p")
            .long("password")
            .value_name("VALUE")
            .help("Set the password to make connections")
            .takes_value(true))
        .arg(Arg::with_name("local-addr")
            .short("l")
            .long("local-address")
            .value_name("IP:PORT")
            .help("Set the local port to proxy")
            .takes_value(true))
        .arg(Arg::with_name("proxy-addr")
            .help("Set the IP:PORT of the proxy")
            .required(true)
            .index(1))
        .arg(Arg::with_name("host")
            .help("Set the hostname to query through the proxy")
            .required(true)
            .index(2))
        .get_matches();

    let local_addr = matches.value_of("local-addr").unwrap_or("127.0.0.1:2222").parse().unwrap();
    let host_info = HostInfo{
        proxy_addr: matches.value_of("proxy-addr").unwrap().parse().unwrap(),
        host: matches.value_of("host").unwrap().to_owned(),
        password: matches.value_of("password").unwrap_or("").to_owned()
    };

    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let client = Client::configure().keep_alive(false).build(&handle);
    let listener = TcpListener::bind(&local_addr, &handle).expect("Failed to bind listener.");
    let conn_stream = listener.incoming()
        .map_err(|e| format!("listen error: {}", e))
        .map(move |(conn, addr)| {
            info!("got connection from {}", addr);
            handle_connection(client.clone(), host_info.clone(), conn).then(move |val| {
                if let Err(e) = val {
                    warn!("{}: {}", addr, e);
                }
                info!("{}: session ended", addr);
                Ok(())
            })
        })
        .buffered(CONCURRENT_CONNS);
    core.run(wait_for_end(conn_stream)).unwrap();
}

/// Generate a Future that drives a new session.
fn handle_connection(
    client: Client<HttpConnector>,
    info: HostInfo,
    conn: TcpStream
) -> Box<Future<Item = (), Error = String>> {
    Box::new(establish_session(&client, &info).and_then(|sess_id| {
        let (read_half, write_half) = conn.split();
        let sess_1 = (client, info, sess_id);
        let sess_2 = sess_1.clone();
        let sess_3 = sess_1.clone();
        let read_future: Box<Future<Item = (), Error = String>> = Box::new(
            ReadStream::new(read_half, MAX_READ_SIZE)
                .map_err(|e| format!("error reading from local socket: {}", e))
                .for_each(move |buf| upload_chunk(sess_1.clone(), buf))
                .and_then(move |_| send_eof(&sess_2)));
        let write_future: Box<Future<Item = (), Error = String>> = Box::new(
            WriteSink::new(write_half)
                .sink_map_err(|e| format!("error sending data: {}", e))
                .send_all(download_stream(&sess_3))
                .map(|_| ()));
        join_all(vec![read_future, write_future]).map(|_| ())
    }))
}

/// Create a new proxy session.
fn establish_session(
    client: &Client<HttpConnector>,
    host_info: &HostInfo
) -> Box<Future<Item = String, Error = String>> {
    let proof = current_proof(&host_info.password);
    Box::new(api_request(client, host_info, "connect", &proof, None).and_then(|body| {
        Ok(String::from(String::from_utf8_lossy(&body)))
    }))
}

/// Send a chunk of data on the session.
fn upload_chunk(info: SessionInfo, chunk: Vec<u8>) -> Box<Future<Item = (), Error = String>> {
    let total_size = chunk.len();
    Box::new(loop_fn(0usize, move |state| {
        let remaining = chunk[state..chunk.len()].to_vec();
        api_request(&info.0, &info.1, "upload", &info.2, Some(remaining)).and_then(move |x| {
            match String::from_utf8_lossy(&x).parse::<usize>() {
                Ok(size) => {
                    Ok(if size + state == total_size {
                        Loop::Break(())
                    } else {
                        Loop::Continue(state + size)
                    })
                },
                Err(_) => Err("invalid response".to_owned())
            }
        })
    }))
}

/// Send an EOF to the remote end.
fn send_eof(info: &SessionInfo) -> Box<Future<Item = (), Error = String>> {
    Box::new(api_request(&info.0, &info.1, "close", &info.2, None).and_then(|_| Ok(())))
}

/// Get a stream of chunks of data from the session.
fn download_stream(info: &SessionInfo) -> Box<Stream<Item = Vec<u8>, Error = String>> {
    Box::new(repeat(info.clone())
        .and_then(|info| {
            api_request(&info.0, &info.1, "download", &info.2, None)
        })
        .and_then(|data| {
            if data.len() == 0 {
                Err("received empty message".to_owned())
            } else {
                if data[0] == 1 {
                    Ok(Some(data[1..data.len()].to_vec()))
                } else {
                    Ok(None)
                }
            }
        })
        .take_while(|info| Ok(info.is_some()))
        .filter(|info| info.as_ref().unwrap().len() > 0)
        .map(|x| x.unwrap()))
}

fn api_request(
    client: &Client<HttpConnector>,
    host_info: &HostInfo,
    api: &str,
    arg: &str,
    data: Option<Vec<u8>>
) -> Box<Future<Item = Vec<u8>, Error = String>> {
    let cache_once = generate_session_id();
    let method = if data.is_some() {
        Method::Post
    } else {
        Method::Get
    };
    let mut req = Request::new(
        method,
        format!("http://{}/{}/{}/{}", host_info.proxy_addr, api, arg, cache_once).parse().unwrap()
    );
    req.headers_mut().set(Host::new(host_info.host.clone(), None));
    req.headers_mut().set(Connection::close());
    if let Some(x) = data {
        req.set_body(x);
    }
    Box::new(client.request(req)
        .map_err(|e| format!("failed to make request: {}", e))
        .and_then(|resp| {
            let status_code = resp.status();
            resp.body().concat2()
                .map_err(|e| format!("failed to read body: {}", e))
                .and_then(move |body| {
                    if status_code != StatusCode::Ok {
                        Err(format!("error from server: {}", String::from_utf8_lossy(&body)))
                    } else {
                        Ok(body.to_vec())
                    }
                })
        }))
}
