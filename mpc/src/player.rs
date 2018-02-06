extern crate bn;
extern crate rand;
extern crate crossbeam;
extern crate rustc_serialize;
extern crate blake2_rfc;
extern crate bincode;
extern crate byteorder;
extern crate sha3;
extern crate web3;
extern crate ipfs_api;
extern crate serde_json;
#[macro_use]
extern crate serde_derive; 

mod protocol;
use self::protocol::*;

mod file;
use self::file::*;

#[cfg(feature = "snark")]
extern crate snark;
use snark::*;

use rand::{SeedableRng, Rng};
use std::fs::{File};
use bincode::SizeLimit::Infinite;
use bincode::rustc_serialize::{encode_into, encode};
use sha3::{Digest, Keccak256};
use rustc_serialize::{Encodable};

use web3::futures::Future;
use web3::contract::*;
use web3::types::{Address, Filter, FilterBuilder, Log, U256, H256, BlockNumber};
use web3::{Transport};
use web3::transports::Http;
use web3::api::BaseFilter;
use web3::Web3;

use ipfs_api::IPFS;

use std::str::FromStr;
use std::env;
use std::time::Duration;
use std::thread;
use std::fmt::Write;

pub const THREADS: usize = 8;
pub const DIRECTORY_PREFIX: &'static str = "/home/compute/";
pub const ASK_USER_TO_RECORD_HASHES: bool = true;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct IPFSAddResponse {
    name: String,
    hash: String,
    size: String
}

fn get_entropy() -> [u32; 8] {
    use blake2_rfc::blake2s::blake2s;

    let mut v: Vec<u8> = vec![];

    {
        let input_from_user = prompt(
            "Please type a random string of text and then press [ENTER] to provide additional entropy."
        );

        let hash = blake2s(32, &[], input_from_user.as_bytes());

        v.extend_from_slice(hash.as_bytes());
    }

    println!("Please wait while Linux fills up its entropy pool...");
    
    {
        let mut linux_rng = rand::read::ReadRng::new(File::open("/dev/random").unwrap());

        for _ in 0..32 {
            v.push(linux_rng.gen());
        }
    }

    assert_eq!(v.len(), 64);

    let hash = blake2s(32, &[], &v);
    let hash = hash.as_bytes();

    let mut seed: [u32; 8] = [0; 8];

    for i in 0..8 {
        use byteorder::{ByteOrder, LittleEndian};

        seed[i] = LittleEndian::read_u32(&hash[(i*4)..]);
    }

    seed
}

fn get_current_state<T: Transport>(contract: &Contract<T>, account: &Address) -> u64 {
    let current_state: U256 = contract.query("currentState", (), *account, Options::default(), BlockNumber::Latest).wait().expect("Error reading current state.");
    current_state.low_u64()
}

fn get_hex_string(bytes: &Vec<u8>) -> String {
    let mut s = String::from("0x");
    for byte in bytes {
        write!(s, "{:02x}", byte).expect("Failed to write byte as hex");
    }
    s
}

fn to_bytes_fixed(vec: &Vec<u8>) -> [u8; 32] {
    let mut arr = [0; 32];
    assert_eq!(32, vec.len());;
    for i in 0..vec.len() {
        arr[i] = vec[i];
    }
    arr
}

fn create_filter<'a>(web3: &Web3<&'a Http>, topic: &str) -> BaseFilter<&'a Http, Log> {
    let mut filter_builder: FilterBuilder = FilterBuilder::default();
    let topic_hash = Keccak256::digest(topic.as_bytes());
    filter_builder = filter_builder.topics(Some(vec![H256::from_str(get_hex_string(&topic_hash.as_slice().to_owned()).as_str()).unwrap()]), None, None, None);
    let filter: Filter = filter_builder.build();
    let create_filter = web3.eth_filter().create_logs_filter(filter);
    create_filter.wait().expect("Filter should be registerable!")
}

fn encode_and_hash<T: Encodable>(obj: &T) -> Vec<u8> {
    Keccak256::digest(&encode(obj, Infinite).unwrap()).as_slice().to_owned()
}

fn await_next_stage(filter: &BaseFilter<&Http, Log>, poll_interval: &Duration) {
    loop {
        let result = filter.poll().wait().expect("New Stage Filter should return result!").expect("Polling result should be valid!");
        if result.len() > 0 {
            let data: &Vec<u8> = &result[result.len()-1].data.0;
            println!("New Stage: {:?}", U256::from(data.as_slice()).low_u64());
            return;
        }
        thread::sleep(*poll_interval);
    }
}

fn await_stage_prepared(filter: &BaseFilter<&Http, Log>, poll_interval: &Duration) {
    loop {
        let result = filter.poll().wait().expect("Stage Prepared Filter should return result!").expect("Polling result should be valid!");
        if result.len() > 0 {
            let data: &Vec<u8> = &result[0].data.0;
            println!("Stage {} prepared", U256::from(data.as_slice()).low_u64());
            return;
        }
        thread::sleep(*poll_interval);
    }
}

fn upload_to_ipfs<T: Encodable>(obj: &T, name: &str, ipfs: &mut IPFS) -> IPFSAddResponse {
    let mut file = File::create(name).unwrap();
    encode_into(obj, &mut file, Infinite).unwrap();
    let result = ipfs.add(name);
    println!("{:?}", String::from_utf8(result.clone()).unwrap());
    serde_json::from_slice(result.as_slice()).unwrap()
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert!(args.len() > 1);
    //get contract file(s)
    let contract_address: Address = args[1].parse().expect("Missing or invalid contract address as first argument!");
    // connect to web3
    println!("Connecting to Web3 instance...");
    let (_eloop, transport) = web3::transports::Http::new("http://localhost:8545").expect("Web3 cannot connect! (http://localhost:8545)");
    let web3 = Web3::new(&transport);
    println!("Successfully connected to web3 instance!");

    let mut ipfs = IPFS::new();
    ipfs.host("http://localhost", 5001);

    let default_account: Address = web3.eth().coinbase().wait().unwrap();

    let contract = Contract::from_json(
        web3.eth(),
        contract_address,
        include_bytes!("../abi.json")
    ).expect("Error loading contract from json!");
    println!("{:?}", contract.address());

    // Create filters:
    let next_stage_filter = create_filter(&web3, "NextStage(uint256)");
    let stage_prepared_filter = create_filter(&web3, "StagePrepared(uint256)");

    // Start protocol
    prompt("Press [ENTER] when you're ready to begin the ceremony.");

    let mut chacha_rng = rand::chacha::ChaChaRng::from_seed(&get_entropy());

    let privkey = PrivateKey::new(&mut chacha_rng);
    let pubkey = privkey.pubkey(&mut chacha_rng);
    let pubkey_encoded: Vec<u8> = encode(&pubkey, Infinite).unwrap();
    let commitment = pubkey.hash();
    let commitment_stringified = get_hex_string(&commitment);
    println!("Commitment: {}", commitment_stringified);
    assert_eq!(66, commitment_stringified.len());

    let duration = Duration::new(1, 0);
    let mut stop = false;
    let mut stage1;
    let mut stage2;
    let mut stage3;
    //TODO: Only Coordinator!
    //TODO: load real r1cs
    let cs = CS::dummy();
    //end of Only Coordinator!
    while !stop {
        match get_current_state(&contract, &default_account) {
            0 => {
                contract.call("start", (), default_account, Options::default()).wait().expect("Start failed!");
                println!("Started!");
                await_next_stage(&next_stage_filter, &duration);
            },
            1 => {
                contract.call("commit", to_bytes_fixed(&commitment.clone()), default_account, Options::default()).wait().expect("Commit failed!");
                println!("Committed!");
                await_next_stage(&next_stage_filter, &duration);
            },
            2 => {
                println!("Pubkey hex: {:?}", get_hex_string(&pubkey_encoded.clone()));
                contract.call("revealCommitment", (pubkey_encoded.clone()), default_account, Options::with(|opt|{opt.gas = Some(U256::from(5000000))})).wait().expect("Error revealing commitment origin!");
                await_next_stage(&next_stage_filter, &duration);
            },
            3 => {
                // TODO: fetch all commitments
                let mut all_commitments: Vec<Vec<u8>> = vec![];
                all_commitments.push(commitment.clone());
                let hash_of_all_commitments = Digest512::from(&all_commitments).unwrap();
                println!("Creating nizks...");
                let nizks = pubkey.nizks(&mut chacha_rng, &privkey, &hash_of_all_commitments);
                println!("Nizks created.");
                let nizks_encoded = encode(&nizks, Infinite).unwrap();
                println!("size of nizks: {} B", nizks_encoded.len());
                contract.call("publishNizks", (nizks_encoded), default_account, Options::with(|opt|{opt.gas = Some(U256::from(5000000))})).wait().expect("Error publishing nizks!");
                await_next_stage(&next_stage_filter, &duration);
            },
            current_stage @ 4 ... 6 => {
                if current_stage == 4 {
                    //TODO: Only Coordinator!
                    println!("Creating stage...");
                    stage1 = Stage1Contents::new(&cs);
                    let stage1_uploaded = upload_to_ipfs(&stage1, "stage1", &mut ipfs);
                    println!("Stage1 size: {} B", stage1_uploaded.size);

                    contract.call("setInitialStage", (stage1_uploaded.hash.into_bytes()), default_account, Options::default()).wait().expect("Error publishing initial stage 1!");
                    await_stage_prepared(&stage_prepared_filter, &duration);
                    //END of Only coordinator
                    println!("Transforming stage...");
                    stage1.transform(&privkey);
                    let stage1_transformed_hash = encode_and_hash(&stage1);
                    //------------------------------------------------------
                    // TODO: continue here with next ipfs upload
                    //------------------------------------------------------
                    contract.call("publishStageResults", (stage1_transformed_hash), default_account, Options::default()).wait().expect("Error publishing results of stage 1!");
                    drop(stage1);
                } else if current_stage == 5 {
                    //TODO: Only Coordinator!
                    println!("Creating stage...");
                    // TODO: load stage1 final from ipfs
                    stage1 = Stage1Contents::new(&cs);
                    stage1.transform(&privkey);
                    stage2 = Stage2Contents::new(&cs, &stage1);
                    let stage2_hash = encode_and_hash(&stage2);
                    contract.call("setInitialStage", (stage2_hash), default_account, Options::default()).wait().expect("Error publishing initial stage 2!");
                    await_stage_prepared(&stage_prepared_filter, &duration);
                    //END of Only coordinator
                    println!("Transforming stage...");
                    stage2.transform(&privkey);
                    let stage2_transformed_hash = encode_and_hash(&stage2);
                    contract.call("publishStageResults", (stage2_transformed_hash), default_account, Options::default()).wait().expect("Error publishing results of stage 2!");
                    drop(stage1);
                    drop(stage2);
                } else {
                    //TODO: Only Coordinator!
                    println!("Creating stage...");
                    stage1 = Stage1Contents::new(&cs);
                    stage1.transform(&privkey);
                    // TODO: load stage2 final from ipfs
                    stage2 = Stage2Contents::new(&cs, &stage1);
                    stage2.transform(&privkey);
                    stage3 = Stage3Contents::new(&cs, &stage2);
                    let stage3_hash = encode_and_hash(&stage3);
                    contract.call("setInitialStage", (stage3_hash), default_account, Options::default()).wait().expect("Error publishing initial stage 3!");
                    await_stage_prepared(&stage_prepared_filter, &duration);
                    //END of Only coordinator
                    println!("Transforming stage...");
                    stage3.transform(&privkey);
                    let stage3_transformed_hash = encode_and_hash(&stage3);
                    contract.call("publishStageResults", (stage3_transformed_hash), default_account, Options::default()).wait().expect("Error publishing results of stage 3!");
                    drop(stage1);
                    drop(stage2);
                    drop(stage3);
                }
                await_next_stage(&next_stage_filter, &duration);
            },
            7 => {
                println!("Protocol finished!");
                stop = true;
            }
            _ => {
                stop = true;
            }
        }
    }
}

