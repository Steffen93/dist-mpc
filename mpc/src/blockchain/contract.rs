use web3::contract::*;
use web3::contract::tokens::{Tokenize, Detokenize};
use web3::futures::Future;
use web3::{Transport};
use web3::types::{Address, BlockNumber, U256};

pub struct ContractWrapper<T:Transport>{
    contract: Contract<T>,
    account: Address
}

impl <T: Transport> ContractWrapper<T>{
    pub fn new(contract: Contract<T>, account: Address) -> ContractWrapper<T>{
        ContractWrapper{
            contract: contract,
            account: account
        }
    }

    pub fn call<P: Tokenize>(&self, method: &str, params: P) -> u64 {
        let tokens = params.into_tokens();
        
        let gas = self.contract.estimate_gas(
            method, 
            tokens.clone().as_slice(), 
            self.account, 
            Options::default())
        .wait().expect("Gas estimation should not fail!");

        self.contract.call(
            method, 
            tokens.as_slice(), 
            self.account, 
            Options::with(|opt|{
                //FIXME!
                opt.gas = Some(U256::from(gas.low_u64()*3))
            }))
        .wait().expect(format!("Error calling contract method {:?}", method).as_str());
        
        gas.low_u64()
    }

    pub fn query<P: Tokenize, R: Detokenize>(&self, method: &str, params: P) -> R {
        self.contract.query(
            method, 
            params, 
            self.account, 
            Options::default(), 
            BlockNumber::Latest)
        .wait().expect(format!("Error querying contract method {:?}", method).as_str())
    }
}