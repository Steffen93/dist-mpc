use dist_files::ipfs::IPFSWrapper;
use super::blockchain::*;
use web3::contract::*;
use web3::futures::Future;
use web3::transports::*;
use web3::types::{Address, U256};
use web3::{Transport, Web3};

use consts::{TOTAL_BYTES, PERFORM_MEASUREMENTS};

use hex;
use json;
use std::fs::File;
use std::io::Read;
use serde_json::value::Value; 

pub struct Manager<T: Transport>{
    pub ipfs: IPFSWrapper,
    pub web3: Web3<T>,
    contract: Option<ContractWrapper<T>>
}

#[derive(Deserialize, Debug)]
struct ContractJson {
    abi: Vec<Value>,
    bytecode: String
}

impl Manager <Http>{
    pub fn new(_web3: Web3<Http>, ipfs_url: &str, ipfs_port: u16) -> Self{
        let _ipfs = IPFSWrapper::new(ipfs_url, ipfs_port);
        Manager{
            ipfs: _ipfs,
            web3: _web3,
            contract: None
        }
    }

    pub fn init_account(&self, index: Option<&str>) -> Address {
        let accounts: Vec<Address> = self.web3.eth().accounts().wait().expect("Error getting accounts!");
        let mut account_index: usize = 0;
        if index.is_some() {
            account_index = index.unwrap().parse().expect("Error reading account index!");
            assert!(account_index < accounts.len());
        }
        accounts[account_index]
    }

    fn deploy_contract(&mut self, path: &str, account: Address) -> Contract<Http> {
        let contract_build: &mut String = &mut String::new();
        File::open(path).expect("Error opening contract json file.").read_to_string(contract_build).expect("Should be readable.");
        let contract_build_json = json::parse(contract_build.as_str()).expect("Error parsing json!");
        let abi = &contract_build_json["abi"];
        let bytecode = &contract_build_json["bytecode"].dump();
        let len = bytecode.len()-1;
        let bytecode_hex: Vec<u8> = hex::decode(&bytecode[3..len]).expect("Unexpected error!");       //skip leading and trailing special characters like "0x..."
        let cs_ipfs = self.ipfs.upload_file("r1cs");
        println!("Size of constraint system : {:?} B", cs_ipfs.size);
        if PERFORM_MEASUREMENTS {
            unsafe {
                TOTAL_BYTES += u64::from_str_radix(&cs_ipfs.size, 10).unwrap();
            }
        }
        let contract = Contract::deploy(self.web3.eth(), &abi.dump().into_bytes()).expect("Abi should be well-formed!")
        .options(Options::with(|opt|{opt.gas = Some(U256::from(4000000))}))
        .execute(bytecode_hex, cs_ipfs.hash.into_bytes(), account).expect("execute failed!").wait().expect("Error after wait!");
        
        contract
    }

    pub fn init_contract(mut self, index: Option<&str>, address: Option<&str>) -> ContractWrapper<Http>{
        let default_account = self.init_account(index);
        let _contract;
        if address.is_some() {
            let contract_address: Address = address.unwrap().parse().expect("Error reading the contract address from the command line!");
            let web3_contract = Contract::from_json(
                self.web3.eth(),
                contract_address,
                include_bytes!("../../abi.json")
            ).expect("Error loading contract from json!");
            _contract = ContractWrapper::new(web3_contract, default_account);
        } else {
            let web3_contract = self.deploy_contract("../blockchain/build/contracts/DistributedMPC.json", default_account);
            _contract = ContractWrapper::new(web3_contract, default_account);
        }
        self.contract = Some(_contract);
        self.contract.unwrap()
    }
}