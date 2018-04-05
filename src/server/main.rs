extern crate clap;
extern crate futures;
extern crate hyper;
extern crate squidtun;
extern crate tokio_core;
extern crate tokio_timer;

#[macro_use]
extern crate log;
extern crate simple_logger;

mod session;
mod server;

use std::sync::{Arc, RwLock};

use clap::{App, Arg};
use hyper::server::Http;
use log::Level;

use server::TunnelService;

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
    Http::new()
        .bind(&listen_addr, move || {
            Ok(TunnelService::new(sessions.clone(), password.clone(), remote_addr))
        })
        .unwrap()
        .run()
        .unwrap();
}
