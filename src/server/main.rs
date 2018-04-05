extern crate clap;
extern crate futures;
extern crate hyper;
extern crate squidtun;
extern crate tokio_core;

#[macro_use]
extern crate log;
extern crate simple_logger;

mod session;
mod server;

use std::sync::{Arc, RwLock};
use std::time::Duration;

use clap::{App, Arg};
use futures::{Future, Stream};
use hyper::server::Http;
use log::Level;
use tokio_core::reactor::{Handle, Interval};

use server::TunnelService;
use session::Session;

fn main() {
    simple_logger::init_with_level(Level::Info).unwrap();

    let matches = App::new("squidtun-server")
        .arg(Arg::with_name("password")
            .short("p")
            .long("password")
            .value_name("VALUE")
            .help("Set the password to make connections")
            .takes_value(true))
        .arg(Arg::with_name("remote")
            .short("r")
            .long("remote")
            .value_name("IP:ADDR")
            .help("Set the IP address to connect to")
            .takes_value(true))
        .arg(Arg::with_name("addr")
            .help("Set the address to listen on")
            .required(true)
            .index(1))
        .get_matches();

    let password = matches.value_of("password").unwrap_or("").to_owned();
    let remote_addr = matches.value_of("remote").unwrap_or("127.0.0.1:22").parse().unwrap();
    let listen_addr = matches.value_of("addr").unwrap().parse().unwrap();

    let sessions = Arc::new(RwLock::new(Vec::new()));
    let sessions_1 = sessions.clone();
    let server = Http::new()
        .bind(&listen_addr, move || {
            Ok(TunnelService::new(sessions.clone(), password.clone(), remote_addr))
        })
        .unwrap();
    server.handle().spawn(timeout_loop(sessions_1, &server.handle()));
    server.run().unwrap();
}

fn timeout_loop(
    sessions: Arc<RwLock<Vec<Session>>>,
    handle: &Handle
) -> Box<Future<Item = (), Error = ()>> {
    Box::new(Interval::new(Duration::from_secs(1), handle).unwrap()
        .map_err(|_| ())
        .for_each(move |_| {
            let sessions: &mut Vec<Session> = &mut sessions.write().unwrap();
            for i in (0..sessions.len()).into_iter().rev() {
                if sessions[i].is_timed_out() {
                    info!("session timed out: {}", sessions[i].id);
                    sessions.remove(i);
                }
            }
            Ok(())
        }))
}
