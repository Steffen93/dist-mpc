//! Events contract wrapper

use ethabi;
use ethabi::{Event};
use web3::Transport;
use web3::api::Eth;

pub struct EventContract<T: Transport> {
    eth: Eth<T>,
    abi: ethabi::Contract
}

impl<T: Transport> EventContract<T> {
    /// Creates new Contract Interface given blockchain address and ABI
    pub fn new(eth: Eth<T>, abi: ethabi::Contract) -> Self {
        EventContract {
          eth,
          abi
        }
    }

    pub fn from_json(eth: Eth<T>, json: &[u8]) -> Result<Self, ethabi::Error> {
        let abi = ethabi::Contract::load(json)?;
        Ok(Self::new(eth, abi))
    } 

    pub fn event(&self, name: &str) -> Result<&Event, ethabi::Error> {
        self.abi.event(name)
    }
}