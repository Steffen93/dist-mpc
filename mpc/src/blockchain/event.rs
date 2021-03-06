extern crate spinner;

use consts::*;

use web3::api::BaseFilter;
use web3::contract::tokens::Detokenize;
use web3::futures::Future;
use web3::types::{Address, Filter, FilterBuilder, Log, H256};
use web3::{Transport, Web3};

use sha3::{Digest, Keccak256};

use spinner::SpinnerBuilder;

use std::fmt::{Write};
use std::str::FromStr;
use std::time::{Duration, Instant};
use std::thread;

use time::{Duration as MDuration};

#[derive(Clone)]
pub struct EventFilterBuilder<T: Transport>{
    web3: Web3<T>
}

impl<T: Transport> EventFilterBuilder<T> {
    pub fn new(web3: Web3<T>) -> Self{
        EventFilterBuilder {
            web3: web3
        }
    }

    pub fn create_filter<F, S>(
        &self, 
        topic: &str, 
        msg: String, 
        cb: F, 
        extra_data: Option<Address>
        ) -> EventFilter<T, F, S> where
        F: Fn(Vec<Log>, Option<Address>) -> Option<S>,
        S: Detokenize
    {
        let mut filter_builder: FilterBuilder = FilterBuilder::default();
        let topic_hash = Keccak256::digest(topic.as_bytes());
        let hex_str = self.get_hex_string(&topic_hash.as_slice().to_owned());
        filter_builder = filter_builder.topics(Some(vec![H256::from_str(hex_str.as_str()).expect("Error parsing topic from string!")]), None, None, None);
        let filter: Filter = filter_builder.build();
        let create_filter = self.web3.eth_filter().create_logs_filter(filter);
        let event_filter = create_filter.wait().expect("Filter should be registerable!");
        EventFilter { 
            filter: event_filter,
            wait_message: msg,
            callback: cb,
            parameter: extra_data
        }
    }

    fn get_hex_string(&self, bytes: &Vec<u8>) -> String {
        let mut s = String::new();
        for byte in bytes {
            write!(s, "{:02x}", byte).expect("Failed to write byte as hex");
        }
        s 
    }
}

pub struct EventFilter<T: Transport, F: Fn(Vec<Log>, Option<Address>) -> Option<S>, S: Detokenize> {
    filter: BaseFilter<T, Log>,
    wait_message: String,
    callback: F,
    parameter: Option<Address>
}

impl<T, F, S> EventFilter<T, F, S> where 
    T: Transport,
    F: Fn(Vec<Log>, Option<Address>) -> Option<S>,
    S: Detokenize
{
    pub fn await(&mut self, duration: &Duration) -> Option<S> where
    S: Detokenize
    {
        let wait_start = Instant::now();
        let spinner = SpinnerBuilder::new(String::from(&*self.wait_message)).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
        loop {
            let result = self.filter.poll().wait().expect("New Stage Filter should return result!").expect("Polling result should be valid!");
            let cb_result = (self.callback)(result, self.parameter);
            if cb_result.is_some() {
                spinner.close();
                if PERFORM_MEASUREMENTS {
                    let duration = MDuration::from_std(wait_start.elapsed());
                    if duration.is_ok() {
                        unsafe {
                            FILTER_OVERHEAD_MS += duration.unwrap().num_milliseconds();
                        }
                    } else {
                        println!("Error in time measurement: Overflow in duration");
                    }
                }
                return cb_result;
            }
            thread::sleep(*duration);
        }

    }
}