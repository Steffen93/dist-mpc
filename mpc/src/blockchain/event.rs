use web3::api::BaseFilter;
use web3::futures::Future;
use web3::types::{Filter, FilterBuilder, Log, H256};
use web3::{Transport, Web3};

use sha3::{Digest, Keccak256};

use std::fmt::{Write};
use std::str::FromStr;
use std::time::Duration;

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

    pub fn create_filter<F: Fn(Vec<Log>) -> bool>(self, topic: &str, cb: F) -> EventFilter<'a, T, F> {
        let mut filter_builder: FilterBuilder = FilterBuilder::default();
        let topic_hash = Keccak256::digest(topic.as_bytes());
        filter_builder = filter_builder.topics(Some(vec![H256::from_str(self.clone().get_hex_string(&topic_hash.as_slice().to_owned()).as_str()).unwrap()]), None, None, None);
        let filter: Filter = filter_builder.build();
        let create_filter = self.web3.eth_filter().create_logs_filter(filter);
        let event_filter = create_filter.wait().expect("Filter should be registerable!");
        EventFilter { 
            filter: event_filter,
            callback: cb
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

pub struct EventFilter<'a, T: 'a + Transport, F: Fn(Vec<Log>)->bool> {
    filter: BaseFilter<&'a T, Log>,
    callback: F
}

impl<'a, T: 'a + Transport, F: Fn(Vec<Log>)-> bool > EventFilter<'a, T, F> {
    pub fn await(self, duration: &Duration){

    }
}