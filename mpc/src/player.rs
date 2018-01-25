#![allow(non_snake_case, dead_code)]

extern crate bn;
extern crate rand;
extern crate crossbeam;
extern crate rustc_serialize;
extern crate blake2_rfc;
extern crate bincode;
extern crate byteorder;
extern crate sha3;
extern crate web3;
extern crate generic_array;
extern crate digest;
extern crate ascii;

mod protocol;
use self::protocol::*;

mod file;
use self::file::*;

use rand::{SeedableRng, Rng};
use std::fs::{File};
use bincode::SizeLimit::Infinite;
use bincode::rustc_serialize::{encode, encode_into, decode_from};

use web3::futures::Future;
use web3::contract::*;
use web3::types::{Address, Filter, FilterBuilder, U256, H256, BlockNumber};
use web3::{Transport};
use std::path::{Path, PathBuf};
use std::io::{Read};
use std::str::FromStr;
use std::env;
use std::time::Duration;
use std::thread;
use std::fmt::Write;
use ascii::{AsciiString};

pub const THREADS: usize = 8;
pub const DIRECTORY_PREFIX: &'static str = "/home/compute/";
pub const ASK_USER_TO_RECORD_HASHES: bool = true;

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

fn get_file_name(path: &String) -> String {
    return String::from(Path::new(path).file_stem().unwrap().to_str().unwrap());
}

fn get_file_path(out_folder: &String, file_name: &String, extension: &str) -> PathBuf {
    let mut output_file_path = PathBuf::from(out_folder.as_str());
    output_file_path.push(file_name);
    output_file_path.set_extension(extension);
    return output_file_path;
}

fn read_file_to_string(path: &PathBuf) -> String {
    let mut output_contract_file = File::open(Path::new(path)).expect("Error opening file!");
    let mut contents = String::new();
    output_contract_file.read_to_string(&mut contents).expect("Error reading file!");
    return contents;
}

fn get_bytes_from_string(string: &mut String) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    while string.len() > 0 {
        let substr: String = string.drain(..2).collect();
        bytes.push(u8::from_str_radix(substr.as_str(), 16).unwrap());
    }
    return bytes;
}

fn get_current_state<T: Transport>(contract: &Contract<T>, account: &Address) -> u64 {
    let currentState: U256 = contract.query("currentState", (), *account, Options::default(), BlockNumber::Latest).wait().expect("Error reading current state.");
    return currentState.low_u64();
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

fn to_string(vec: &Vec<u8>) -> String {
    let mut string = String::new();
    for elem in vec {
        string.push(*elem as char);
    }
    string
}

fn to_bytes(string: String) -> Vec<u8> {
    let mut vec: Vec<u8> = Vec::new();
    for char in string.chars() {
        vec.push(char as u8);
    }
    vec
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert!(args.len() > 1);
    //get contract file(s)
    let contract_address: Address = args[1].parse().unwrap();
    // connect to web3
    println!("Connecting to Web3 instance...");
    let (_eloop, transport) = web3::transports::Http::new("http://localhost:8545").expect("Web3 cannot connect! (http://localhost:8545)");
    let web3 = web3::Web3::new(&transport);
    println!("Successfully connected to web3 instance!");

    let default_account: Address = web3.eth().coinbase().wait().unwrap();

    let contract = Contract::from_json(
        web3.eth(),
        contract_address,
        include_bytes!("../abi.json")
    ).expect("Error loading contract from json!");
    println!("{:?}", contract.address());

    // contract magic:
     
    let mut filterBuilder: FilterBuilder = FilterBuilder::default();
    filterBuilder = filterBuilder.topics(Some(vec![H256::from_str("0xf2f13d712bddc038fd1341d24bad63155a3e68fb5b398cb8f170cd736c277505").unwrap()]), None, None, None);
    let filter: Filter = filterBuilder.build();
    let createFilter = web3.eth_filter().create_logs_filter(filter);
    let baseFilter = createFilter.wait().expect("Filter should be registerable!");

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
    while !stop {
        match get_current_state(&contract, &default_account) {
            0 => {
                contract.call("start", (), default_account, Options::default()).wait().expect("Start failed!");
                println!("Started!");
                loop {
                    let result = baseFilter.poll().wait().expect("Base Filter should return result!").expect("Polling result should be valid!");
                    if result.len() > 0 {
                        let data: &Vec<u8> = &result[0].data.0;
                        println!("New Stage: {:?}", U256::from(data.as_slice()).low_u64());
                        break;
                    }
                    thread::sleep(duration);
                }
            },
            1 => {
                //println!("Hashed commitment: {:?}", hashed_commitment);
                contract.call("commit", to_bytes_fixed(&commitment.clone()), default_account, Options::with(|opt| {opt.gas = Some(U256::from(5000000))})).wait().expect("Commit failed!");
                println!("Committed!");
                loop {
                    let result = baseFilter.poll().wait().expect("Base Filter should return result!").expect("Polling result should be valid!");
                    if result.len() > 0 {
                        let data: &Vec<u8> = &result[0].data.0;
                        println!("New Stage: {:?}", U256::from(data.as_slice()).low_u64());
                        break;
                    }
                    thread::sleep(duration);
                }
            },
            2 => {
                contract.call("publishPlayerData", (String::from("Thisismynizks"), pubkey_encoded.clone()), default_account, Options::with(|opt| {opt.gas = Some(U256::from(5000000))})).wait().expect("Error publishing commitment origin!");
                loop {
                    let result = baseFilter.poll().wait().expect("Base Filter should return result!").expect("Polling result should be valid!");
                    if result.len() > 0 {
                        let data: &Vec<u8> = &result[0].data.0;
                        println!("New Stage: {:?}", U256::from(data.as_slice()).low_u64());
                        break;
                    }
                    thread::sleep(duration);
                }
            },
            _ => {
                stop = true;
            }
        }
    }

    let (hash_of_commitments, mut stage1, prev_msg_hash): (Digest512, Stage1Contents, Digest256) = read_file(
        "A",
        &format!("Commitment: {:?}\n\n\
                  Write this commitment down on paper.\n\n\
                  Then type the above commitment into the networked machine.\n\n\
                  The networked machine should produce file 'A'.\n\n\
                  When file 'A' is in ready, press [ENTER].", commitment_stringified),
        |f, p| -> Result<_, bincode::rustc_serialize::DecodingError> {
            let hash_of_commitments: Digest512 = try!(decode_from(f, Infinite));
            let stage: Stage1Contents = try!(decode_from(f, Infinite));

            Ok((hash_of_commitments, stage, p.unwrap()))
        }
    );

    let nizks = pubkey.nizks(&mut chacha_rng, &privkey, &hash_of_commitments);

    reset();
    println!("Please wait while disc 'B' is computed... This should take 30 minutes to an hour.");
    stage1.transform(&privkey);

    let (mut stage2, prev_msg_hash): (Stage2Contents, Digest256) = exchange_file(
        "B",
        "C",
        |f| {
            try!(encode_into(&pubkey, f, Infinite));
            try!(encode_into(&nizks, f, Infinite));
            try!(encode_into(&stage1, f, Infinite));

            encode_into(&prev_msg_hash, f, Infinite)
        },
        |f, p| -> Result<(Stage2Contents, Digest256), bincode::rustc_serialize::DecodingError> {
            let stage2 = try!(decode_from(f, Infinite));

            Ok((stage2, p.unwrap()))
        }
    );

    drop(stage1);

    reset();
    println!("Please wait while disc 'D' is computed... This should take 45 to 90 minutes.");
    stage2.transform(&privkey);

    let (mut stage3, prev_msg_hash): (Stage3Contents, Digest256) = exchange_file(
        "D",
        "E",
        |f| {
            try!(encode_into(&stage2, f, Infinite));

            encode_into(&prev_msg_hash, f, Infinite)
        },
        |f, p| -> Result<(Stage3Contents, Digest256), bincode::rustc_serialize::DecodingError> {
            let stage3 = try!(decode_from(f, Infinite));

            Ok((stage3, p.unwrap()))
        }
    );

    drop(stage2);

    reset();
    println!("Please wait while disc 'F' is computed... This should take 15-30 minutes.");
    stage3.transform(&privkey);

    write_file(
        "F",
        |f| {
            try!(encode_into(&stage3, f, Infinite));

            encode_into(&prev_msg_hash, f, Infinite)
        },
    );
}
