#![allow(non_snake_case, dead_code)]

extern crate bn;
extern crate rand;
extern crate snark;
extern crate crossbeam;
extern crate rustc_serialize;
extern crate blake2_rfc;
extern crate bincode;
extern crate byteorder;
extern crate hyper;
extern crate tokio_core;
 
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate time;
extern crate ansi_term;

#[macro_use]
mod protocol;
use self::protocol::*;

mod consts;
use self::consts::*;

use snark::*;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::str::FromStr;
use rustc_serialize::{Decodable, Encodable};
use rustc_serialize::hex::ToHex;
use bincode::SizeLimit::Infinite;
use bincode::rustc_serialize::{encode_into, decode_from};
use std::time::Duration;

use hyper::{Method, Client, Request, Uri};

const RPC_ENDPOINT: &'static str = "127.0.0.1:8545";
pub const THREADS: usize = 128;

fn main() {
    {
        // Initialize the logger.
        let start_time = time::now();
        let format = move |record: &log::LogRecord| {
            use ansi_term::Colour::*;

            let since = time::now() - start_time;
            let hours = since.num_hours();
            let minutes = since.num_minutes() % 60;
            let seconds = since.num_seconds() % 60;

            let level = match record.level() {
                a @ log::LogLevel::Warn => {
                    format!("{}", Yellow.bold().paint(format!("{}", a)))
                },
                a @ log::LogLevel::Error => {
                    format!("{}", Red.bold().paint(format!("{}", a)))
                },
                a @ _ => {
                    format!("{}", a)
                }
            };

            format!("({}) [T+{}h{}m{}s]: {}", level, hours, minutes, seconds, record.args())
        };

        let mut builder = env_logger::LogBuilder::new();
        builder.format(format).filter(None, log::LogLevelFilter::Info);
        builder.init().unwrap();
    }

    info!("Checking Blockchain Connection at {}", RPC_ENDPOINT);
    let mut core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let client = Client::new(&handle);
    let request = Request::new(Method::Post, Uri::from_str(RPC_ENDPOINT).unwrap());
    let res = client.request(request).body().send();
    match res {
        Ok(res) => info!("Response: {}", res.status),
        Err(e) => info!("Error: {}", e)
    }
}
