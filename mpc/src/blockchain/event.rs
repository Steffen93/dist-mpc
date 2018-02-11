extern crate spinner;

use web3::api::BaseFilter;
use web3::futures::Future;
use web3::types::{Address, Filter, FilterBuilder, Log, H256};
use web3::{Transport, Web3};

use sha3::{Digest, Keccak256};

use spinner::SpinnerBuilder;

use std::fmt::{Write};
use std::str::FromStr;
use std::time::Duration;
use std::thread;

#[derive(Clone, Copy)]
pub struct EventFilterBuilder<'a, T: 'a + Transport>{
    web3: &'a Web3<&'a T>
}

impl<'a, T: 'a + Transport> EventFilterBuilder<'a, T> {
    pub fn new(web3: &'a Web3<&'a T>) -> EventFilterBuilder<T>{
        EventFilterBuilder {
            web3: web3
        }
    }

    pub fn create_filter<F: Fn(Vec<Log>, Option<Address>) -> bool>(&self, topic: &str, msg: String, cb: F, extra_data: Option<Address>) -> EventFilter<'a, T, F> {
        let mut filter_builder: FilterBuilder = FilterBuilder::default();
        let topic_hash = Keccak256::digest(topic.as_bytes());
        filter_builder = filter_builder.topics(Some(vec![H256::from_str(self.clone().get_hex_string(&topic_hash.as_slice().to_owned()).as_str()).unwrap()]), None, None, None);
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

    fn get_hex_string(self, bytes: &Vec<u8>) -> String {
        let mut s = String::from("0x");
        for byte in bytes {
            write!(s, "{:02x}", byte).expect("Failed to write byte as hex");
        }
        s 
    }
}

pub struct EventFilter<'a, T: 'a + Transport, F: Fn(Vec<Log>, Option<Address>) -> bool> {
    filter: BaseFilter<&'a T, Log>,
    wait_message: String,
    callback: F,
    parameter: Option<Address>
}

impl<'a, T, F> EventFilter<'a, T, F> where 
    T: 'a + Transport,
    F: Fn(Vec<Log>, Option<Address>) -> bool
{
    pub fn await(&mut self, duration: &Duration){
        let spinner = SpinnerBuilder::new(String::from(&*self.wait_message)).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
        loop {
            let result = self.filter.poll().wait().expect("New Stage Filter should return result!").expect("Polling result should be valid!");
            if (self.callback)(result, self.parameter) {
                spinner.close();
                return;
            }
            thread::sleep(*duration);
        }

    }
}