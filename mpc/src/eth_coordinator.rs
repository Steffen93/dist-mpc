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
extern crate futures;
 
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

use hyper::{Client};
use hyper::header::{ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel, Attr, Value};
use futures::future::*;

const RPC_ENDPOINT: &'static str = "http://127.0.0.1:8545";
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
    let client = Client::new();
    let mut res = client.post(RPC_ENDPOINT).body("{\"jsonrpc\":\"2.0\",  \"method\":\"net_version\", \"params\":[], \"id\":67}").header(ContentType(Mime(TopLevel::Application, SubLevel::Json, vec![(Attr::Charset, Value::Utf8)]))).send().unwrap();
    let mut result: &mut String = &mut String::new();
    res.read_to_string(result).unwrap();
    info!("Response status: {}", res.status);
    info!("Response: {}", result);
}
