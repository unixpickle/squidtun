extern crate clap;
extern crate futures;
extern crate hyper;
extern crate squidtun;
extern crate tokio_core;
extern crate tokio_io;

mod future_util;

use std::net::SocketAddr;

use clap::{App, Arg};
use futures::{Future, IntoFuture, Stream};
use hyper::StatusCode;
use hyper::client::{Client, HttpConnector};
use squidtun::current_proof;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;

use future_util::wait_for_end;

const CONCURRENT_CONNS: usize = 5;

#[derive(Clone, Debug)]
struct HostInfo {
    proxy_addr: SocketAddr,
    host: String,
    password: String
}

fn main() {
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
    let client = Client::new(&handle);
    let listener = TcpListener::bind(&local_addr, &handle).expect("Failed to bind listener.");
    let conn_stream = listener.incoming()
        .map_err(|e| format!("listen error: {}", e))
        .map(move |(conn, _)| {
            handle_connection(&client, &host_info, &conn)
        })
        .buffered(CONCURRENT_CONNS);
    core.run(wait_for_end(conn_stream)).unwrap();
}

/// Generate a Future that drives a new session.
fn handle_connection(
    client: &Client<HttpConnector>,
    info: &HostInfo,
    conn: &TcpStream
) -> Box<Future<Item = (), Error = String>> {
    Box::new(establish_session(client, info).and_then(|sess_id| {
        // TODO: start reader and writer.
        Ok(()).into_future()
    }))
}

/// Create a new proxy session.
fn establish_session(
    client: &Client<HttpConnector>,
    host_info: &HostInfo
) -> Box<Future<Item = String, Error = String>> {
    let conn_url = format!("http://{}/connect/{}", host_info.proxy_addr,
        current_proof(&host_info.password));
    // TODO: put hostname into request!
    Box::new(client.get(conn_url.parse().unwrap())
        .map_err(|e| format!("failed to make connect request: {}", e))
        .and_then(|response| {
            let status_code = response.status();
            response.body().concat2()
                .map_err(|e| format!("failed to read body: {}", e))
                .and_then(move |body| {
                    let str_body = String::from(String::from_utf8_lossy(&body));
                    if status_code == StatusCode::Ok {
                        Ok(str_body)
                    } else {
                        Err(str_body)
                    }
                })
        }))
}
