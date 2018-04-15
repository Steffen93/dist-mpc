extern crate bincode;
extern crate blake2_rfc;
extern crate bn;
extern crate byteorder;
extern crate crossbeam;
extern crate ethabi;
extern crate ethereum_types;
extern crate hex;
extern crate ipfs_api;
extern crate json;
extern crate rand;
extern crate rustc_serialize;
extern crate serde_json;
extern crate sha3;
extern crate spinner;
extern crate time;
extern crate web3;

#[macro_use]
extern crate clap;
use clap::{App};

#[macro_use]
extern crate serde_derive; 

#[cfg(feature = "snark")]
extern crate snark;
use snark::*;

mod protocol; 
use self::protocol::*;
use protocol::{Transform, Verify};

mod blockchain;
use self::blockchain::*;

mod manager;
use self::manager::*;

mod dist_files;
use self::dist_files::*;

mod consts;
use self::consts::*;

use spinner::SpinnerBuilder;
use rustc_serialize::{Encodable, Decodable};

use ethereum_types::{Address, H256, U256};
use web3::api::Eth;
use web3::contract::tokens::{Tokenize};
use web3::futures::Future;
use web3::types::{Log, TransactionReceipt};
use web3::{Transport, Web3};
use web3::transports::Http;

use rand::{SeedableRng, Rng};

use std::time::{Duration, Instant};
use std::io::{self};
use std::fs::File;
use std::env::var;

use time::Duration as MDuration;

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
        let wait_start = Instant::now();
        let mut linux_rng = rand::read::ReadRng::new(File::open("/dev/random").unwrap());
        for _ in 0..32 {
            v.push(linux_rng.gen());
        }
        if PERFORM_MEASUREMENTS {
            let duration = MDuration::from_std(wait_start.elapsed());
            if duration.is_ok() {
                unsafe {
                    INPUT_OVERHEAD_MS += duration.unwrap().num_milliseconds();
                }
            } else {
                    println!("Error in time measurement: Overflow in duration");
            }
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

fn get_current_state<T: Transport>(contract: &ContractWrapper<T>) -> u64 {
    let current_state: U256 = contract.query("currentState", ());
    current_state.low_u64()
}

fn to_bytes_fixed(vec: &Vec<u8>) -> [u8; 32] {
    let mut arr = [0; 32];
    assert_eq!(32, vec.len());;
    for i in 0..vec.len() {
        arr[i] = vec[i];
    }
    arr
}

fn download_stage<P, S, T>(contract: &ContractWrapper<T>, method: &str, params: P, ipfs: &mut IPFSWrapper) -> S where 
    P: Tokenize,
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let spinner = SpinnerBuilder::new("Querying stage hash from Ethereum...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
    let stage_hash: Vec<u8> = contract.query(method, params);
    spinner.message(format!("Downloading stage from IPFS (hash: {:?})", String::from_utf8(stage_hash.clone()).unwrap()));
    let stage = ipfs.download_stage(String::from_utf8(stage_hash).unwrap().as_str());
    spinner.close();
    stage
}

fn transform_and_upload<S, T>(stage: &mut S, privkey: &PrivateKey, pubkey: &PublicKey, contract: &ContractWrapper<T>, file_name: &str, ipfs: &mut IPFSWrapper) -> H256 where
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let prev_stage = &stage.clone();
    let spinner = SpinnerBuilder::new("Transforming stage...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
    stage.transform(privkey);
    assert!(stage.verify_transform(prev_stage, pubkey), "Invalid stage transformation!");
    spinner.close();
    upload_object(stage, contract, "publishStageResults", file_name, ipfs)
}

fn init_stage_and_upload<S, T>(stage: &mut S, privkey: &PrivateKey, pubkey: &PublicKey, contract: &ContractWrapper<T>, file_name: &str, ipfs: &mut IPFSWrapper) -> H256 where
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let prev_stage = &stage.clone();
    let spinner = SpinnerBuilder::new("Transforming stage...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
    stage.transform(privkey);
    assert!(stage.verify_transform(prev_stage, pubkey), "Invalid stage transformation!");
    spinner.message("Uploading stage and transformation to IPFS...".into());
    let prev_stage_ipfs = ipfs.upload_object(prev_stage, file_name);
    let stage_ipfs = ipfs.upload_object(stage, format!("{}_transformed", file_name).as_str());
    spinner.message("Publishing stage and transformation hashes to Ethereum...".into());
    let transaction_hash = contract.call("setInitialStage", (prev_stage_ipfs.hash.into_bytes(), stage_ipfs.hash.into_bytes()));
    spinner.close();
    transaction_hash
}

fn measure_gas_usage<T: Transport>(hash: H256, eth: &Eth<T>) {
    if PERFORM_MEASUREMENTS {
        let receipt: Option<TransactionReceipt> = eth.transaction_receipt(hash).wait().expect("Call result error!");
        if receipt.is_none(){
                println!("No receipt for transaction hash {:?}", hash);
        }
        else {
            let gas: u64 = receipt.unwrap().gas_used.low_u64();
                println!("GAS USED IN TRANSACTION: {}", gas);
            unsafe {
                TOTAL_GAS += gas;
            }
        }
    }
}

fn upload_object<S, T>(object: &mut S, contract: &ContractWrapper<T>, method_name: &str, file_name: &str, ipfs: &mut IPFSWrapper) -> H256 where
    S: Encodable,
    T: Transport
{
    let spinner = SpinnerBuilder::new(format!("Uploading {:?} to ipfs ...", file_name)).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
    let stage_ipfs = ipfs.upload_object(object, file_name);
    let transaction_hash = contract.call(method_name, stage_ipfs.hash.into_bytes());
    spinner.close();    
    transaction_hash
}

fn prompt(s: &str) -> String {
    if NON_INTERACTIVE {
        return "".into();
    }
    let wait_start = Instant::now();
    loop {
        let mut input = String::new();
        //reset();
            println!("{}", s);
            println!("\x07");

        if io::stdin().read_line(&mut input).is_ok() {
                println!("Please wait...");
            if PERFORM_MEASUREMENTS {
                let duration = MDuration::from_std(wait_start.elapsed());
                if duration.is_ok() {
                    unsafe {
                        INPUT_OVERHEAD_MS += duration.unwrap().num_milliseconds();
                    }
                } else {
                        println!("Error in time measurement: Overflow in duration");
                }
            }
            return (&input[0..input.len()-1]).into();
        }
    }
}

fn is_coordinator<T: Transport>(contract: &ContractWrapper<T>, account: Address) -> bool {
    account == contract.query("players", 0)
}

fn get_players<T: Transport>(contract: &ContractWrapper<T>) -> Vec<Address> {
    let mut players: Vec<Address> = vec![];
    let number_of_players: u64 = contract.query("getNumberOfPlayers", ());
    for i in 0..number_of_players { 
        let player: Address = contract.query("players", i);
        players.push(player);
    }
    players
}

fn get_previous_player(players: Vec<Address>, player: Address) -> Option<Address> {
    for i in 0..players.len() {
        if i == 0 && players[i] == player {
            return None;
        }
        if players[i] == player {
            return Some(players[i-1]);
        }
    }
    None
}

fn fetch_all_commitments<T: Transport>(contract: &ContractWrapper<T>, players: Vec<Address>) -> Vec<Vec<u8>> {
    let mut all_commitments : Vec<Vec<u8>> = vec![];
    for player in players {
        let commitment: Vec<u8> = contract.query("getCommitment", player);
        all_commitments.push(commitment);
    }
    all_commitments
}

fn verify_all_nizks_valid<T: Transport>(contract: &ContractWrapper<T>, players: Vec<Address>, hash_of_all_commitments: &Digest512, ipfs: &mut IPFSWrapper) {
    for i in 0..players.len() {
        let player_index: u64 = i as u64; 
        let nizks_bin: Vec<u8> = contract.query("getNizks", player_index);
        let pubkey_bin: Vec<u8> = contract.query("getPublicKey", player_index);
        let nizks: PublicKeyNizks = ipfs.download_object(String::from_utf8(nizks_bin).expect("Should be valid IPFS hash!").as_str());
        let pubkey: PublicKey = ipfs.download_object(String::from_utf8(pubkey_bin).expect("Should be valid IPFS hash!").as_str());
        assert!(nizks.is_valid(&pubkey, hash_of_all_commitments), format!("Nizks was invalid for player {}! Aborting.", i));
    }
}

fn main() {
    let program_start = Instant::now();
    let host_opt = var(HOST_ENV_KEY);
    let mut host = String::from(DEFAULT_HOST);
    if host_opt.is_ok() {
            host = host_opt.unwrap();
                println!("Using host from environment variable: {:?}", host);
    }

    let mut call_transactions: Vec<H256> = vec![];

    let yaml = load_yaml!("../player.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let account_index = matches.value_of("account");
    let contract_address = matches.value_of("contract");

        println!("Initializing Web3 and IPFS...");
    let (_eloop, transport) = Http::new(format!("http://{}:8545", host).as_str()).expect("Error connecting to web3 instance!");
    let manager: Manager<Http> = Manager::new(Web3::new(transport), format!("http://{}", host).as_str(), 5001);

    let web3: Web3<Http> = manager.web3.clone();
    let mut ipfs: IPFSWrapper = IPFSWrapper::new(format!("http://{}", host).as_str(), 5001);
        println!("Successfully initialized.");
    
    let contract = manager.init_contract(account_index, contract_address);
    let default_account = contract.account(); 
        println!("Your account used: {:?}", default_account);
        println!("Contract address: {:?}", contract.address());

    let filter_builder = EventFilterBuilder::new(web3.clone()); 
    let poll_interval = Duration::new(1, 0);
    let mut player_joined_filter = filter_builder.create_filter("PlayerJoined(address)", "Waiting for player joining...".into(), player_joined_cb, Some(default_account));
    let mut next_stage_filter = filter_builder.create_filter("NextStage(uint256)", "Waiting for next stage to start...".into(), next_stage_cb, None);
    
    // IF CURRENT ACCOUNT IS NOT A PLAYER, JOIN!
    let mut players: Vec<Address> = get_players(&contract);
    if players.contains(&default_account) {
            println!("You are a player in the protocol already, continuing...");
    } else {
            println!("Welcome new player! Joining now...");
        let transaction_hash = contract.call("join", ());
        if PERFORM_MEASUREMENTS {
            call_transactions.push(transaction_hash);
        }
        player_joined_filter.await(&poll_interval);    
        players = get_players(&contract);
    }
    let previous_player: Option<Address> = get_previous_player(players.clone(), default_account);
    let prev_player_str: String;
    if previous_player.is_some() {
        prev_player_str = format!("{:?}", previous_player.unwrap());
    } else {
        prev_player_str = "nobody".into();   
    }
    let mut stage_result_published_filter = filter_builder.create_filter("StageResultPublished(address,bytes)", format!("Waiting for {:?} to publish results...", prev_player_str), stage_result_cb, previous_player); 
    let mut chacha_rng = rand::chacha::ChaChaRng::from_seed(&get_entropy());

    let privkey = PrivateKey::new(&mut chacha_rng);
    let mut pubkey = privkey.pubkey(&mut chacha_rng);
    let commitment = pubkey.hash();

    let cs_hash: Vec<u8> = contract.query("getConstraintSystem", ());
    let cs = ipfs.download_cs(String::from_utf8(cs_hash).expect("Not a valid utf8 string").as_str());
    let mut stop = false;
    let mut stage1: Stage1Contents;
    let mut stage2: Stage2Contents;
    let mut stage3: Stage3Contents;
        println!("!!! READ CAREFULLY !!! Beyond this point, the program MUST NOT BE STOPPED OR INTERRUPTED until the end of the protocol.");
        println!("If it is interrupted anyways, there is no way to restart the protocol using the same Smart Contract!");
    prompt("Press [ENTER] when you are ready to start the protocol.");
    while !stop {
        match get_current_state(&contract) {
            0 => {
                if is_coordinator(&contract, default_account){
                    prompt("You are the coordinator. Press [ENTER] to start the protocol.");
                    let transaction_hash = contract.call("commit", to_bytes_fixed(&commitment.clone()));
                    if PERFORM_MEASUREMENTS {
                        call_transactions.push(transaction_hash);
                    }
                } else {
                        println!("You are not the coordinator. The protocol will start as the coordinator decides.");
                }
                next_stage_filter.await(&poll_interval);
                players = get_players(&contract);
            },
            1 => {
                if !is_coordinator(&contract, default_account){
                    let transaction_hash = contract.call("commit", to_bytes_fixed(&commitment.clone()));
                    if PERFORM_MEASUREMENTS {
                        call_transactions.push(transaction_hash);
                    }
                }
                next_stage_filter.await(&poll_interval);
                    println!("All players committed. Proceeding to next round.");
            },
            2 => {
                let transaction_hash = upload_object(&mut pubkey, &contract, "revealCommitment", "publicKey", &mut ipfs);
                if PERFORM_MEASUREMENTS {
                    call_transactions.push(transaction_hash);
                }
                    println!("Public Key revealed! Waiting for other players to reveal...");
                next_stage_filter.await(&poll_interval);
                    println!("All players revealed their commitments. Proceeding to next round.");
            },
            3 => {
                let mut all_commitments = fetch_all_commitments(&contract, players.clone());
                let hash_of_all_commitments = Digest512::from(&all_commitments).unwrap();
                    println!("Creating nizks...");
                let mut nizks = pubkey.nizks(&mut chacha_rng, &privkey, &hash_of_all_commitments);
                    println!("Nizks created.");
                let transaction_hash = upload_object(&mut nizks, &contract, "publishNizks", "nizks", &mut ipfs);
                if PERFORM_MEASUREMENTS {
                    call_transactions.push(transaction_hash);
                }
                next_stage_filter.await(&poll_interval);
                    println!("All nizks published. Checking validity...");
                verify_all_nizks_valid(&contract, players.clone(), &hash_of_all_commitments, &mut ipfs);
            },
            4 => {
                if is_coordinator(&contract, default_account) {
                        println!("Creating stage...");
                    stage1 = Stage1Contents::new(&cs);
                    let transaction_hash = init_stage_and_upload(&mut stage1, &privkey, &pubkey, &contract, "stage1", &mut ipfs);
                    if PERFORM_MEASUREMENTS {
                        call_transactions.push(transaction_hash);
                    }
                    drop(stage1);
                } else {
                    let stage_hash = stage_result_published_filter.await(&poll_interval).unwrap();
                    let mut stage1: Stage1Contents;
                    stage1 = ipfs.download_stage(String::from_utf8(stage_hash).expect("Should be valid IPFS hash").as_str());
                    let transaction_hash = transform_and_upload(&mut stage1, &privkey, &pubkey, &contract, "stage1_transformed", &mut ipfs);
                    if PERFORM_MEASUREMENTS {
                        call_transactions.push(transaction_hash);
                    }
                    drop(stage1);
                }
                next_stage_filter.await(&poll_interval);
            },
            5 => {
                if is_coordinator(&contract, default_account) {
                        println!("Creating stage...");
                    stage1 = download_stage(&contract, "getLatestTransformation", (), &mut ipfs);
                    stage2 = Stage2Contents::new(&cs, &stage1);
                    drop(stage1);
                    let transaction_hash = init_stage_and_upload(&mut stage2, &privkey, &pubkey, &contract, "stage2", &mut ipfs);
                    if PERFORM_MEASUREMENTS {
                        call_transactions.push(transaction_hash);
                    }
                    drop(stage2);
                } else {
                    let stage_hash = stage_result_published_filter.await(&poll_interval).unwrap();
                    let mut stage2: Stage2Contents;
                    stage2 = ipfs.download_stage(String::from_utf8(stage_hash).expect("Should be valid IPFS hash").as_str());
                    let transaction_hash = transform_and_upload(&mut stage2, &privkey, &pubkey, &contract, "stage2_transformed", &mut ipfs);
                    if PERFORM_MEASUREMENTS {
                        call_transactions.push(transaction_hash);
                    }
                    drop(stage2);
                }
                next_stage_filter.await(&poll_interval);
            },
            6 => {
                if is_coordinator(&contract, default_account) {
                        println!("Creating stage...");
                    stage2 = download_stage(&contract, "getLatestTransformation", (), &mut ipfs);
                    stage3 = Stage3Contents::new(&cs, &stage2);
                    drop(stage2); 
                    let transaction_hash = init_stage_and_upload(&mut stage3, &privkey, &pubkey, &contract, "stage3", &mut ipfs);
                    if PERFORM_MEASUREMENTS {
                        call_transactions.push(transaction_hash);
                    }
                    drop(stage3);
                } else {
                    let stage_hash = stage_result_published_filter.await(&poll_interval).unwrap();
                    let mut stage3: Stage3Contents;
                    stage3 = ipfs.download_stage(String::from_utf8(stage_hash).expect("Should be valid IPFS hash").as_str());
                    let transaction_hash = transform_and_upload(&mut stage3, &privkey, &pubkey, &contract, "stage3_transformed", &mut ipfs);
                    if PERFORM_MEASUREMENTS {
                        call_transactions.push(transaction_hash);
                    }
                    drop(stage3);
                }
                next_stage_filter.await(&poll_interval);
            },
            7 => {
                    println!("Protocol finished! You can now exit this program and run the verifier to create the keypair.");
                if PERFORM_MEASUREMENTS {
                    let total_secs: i64 = program_start.elapsed().as_secs() as i64;
                        println!("Total program runtime: {:?}s", program_start.elapsed().as_secs());
                    unsafe{
                        let filter_overhead_secs: f64 = FILTER_OVERHEAD_MS as f64 / 1000 as f64;
                            println!("Overhead caused by waiting for the blockchain: {}s ({:.2}%)", filter_overhead_secs as i64, (filter_overhead_secs / total_secs as f64) * 100 as f64);        
                        let input_overhead_secs: f64 = INPUT_OVERHEAD_MS as f64 / 1000 as f64;
                            println!("Overhead caused by waiting for inputs: {}s ({:.2}%)", input_overhead_secs as i64, (input_overhead_secs / total_secs as f64) * 100 as f64);        
                        let execution_secs: f64 = total_secs as f64 - filter_overhead_secs - input_overhead_secs;
                            println!("Net execution time of the protocol: {}s ({:.2}%)", execution_secs as i64, (execution_secs / total_secs as f64) * 100 as f64);
                            println!("Share of net execution time / blockchain overhead ignoring input overhead: {:.2}%/{:.2}%", (execution_secs / (total_secs as f64 - input_overhead_secs) as f64) * 100 as f64, (filter_overhead_secs / (total_secs as f64 - input_overhead_secs) as f64) * 100 as f64);
                    }
                    unsafe {
                            println!("Total amount of bytes written to IPFS by this peer: {:?} B", TOTAL_BYTES);
                    }
                    for hash in call_transactions.clone() {
                        measure_gas_usage(hash, &web3.eth());
                    }
                    unsafe {
                            println!("Total amount of gas used by this peer (excluding contract creation): {:?}", TOTAL_GAS);
                    }
                    print_for_benchmarks(total_secs, call_transactions.clone(), &web3.eth());
                }
                stop = true;
            },
            _ => {
                stop = true;
            }
        }
    }
}

fn print_for_benchmarks<T: Transport>(total_secs: i64, transactions: Vec<H256>, eth: &Eth<T>){
    unsafe {
        let filter_overhead_secs: f64 = FILTER_OVERHEAD_MS as f64 / 1000 as f64;
        let input_overhead_secs: f64 = INPUT_OVERHEAD_MS as f64 / 1000 as f64;
        let execution_secs: f64 = total_secs as f64 - filter_overhead_secs - input_overhead_secs;
        let mut gas: Vec<u64> = vec![];
        if transactions.len() < 7{
            gas.push(0);
        }
        for hash in transactions {
            let receipt: Option<TransactionReceipt> = eth.transaction_receipt(hash).wait().expect("Call result error!");
            if receipt.is_none(){
                println!("No receipt for transaction hash {:?}", hash);
            }
            else {
                let gas_used: u64 = receipt.unwrap().gas_used.low_u64();
                gas.push(gas_used);
            }
        }
        assert_eq!(gas.len(), 7);
        println!("{},{},{:.2},{},{:.2},{},{:.2},{:.2}/{:.2},{},{},{},{},{},{},{},{},{}", 
            total_secs, 
            filter_overhead_secs as i64, 
            (filter_overhead_secs / total_secs as f64) * 100 as f64,
            input_overhead_secs as i64, 
            (input_overhead_secs / total_secs as f64) * 100 as f64,
            execution_secs as i64, 
            (execution_secs / total_secs as f64) * 100 as f64,
            (execution_secs / (total_secs as f64 - input_overhead_secs) as f64) * 100 as f64,
            (filter_overhead_secs / (total_secs as f64 - input_overhead_secs) as f64) * 100 as f64,
            TOTAL_BYTES,
            TOTAL_GAS,
            gas[0],
            gas[1],
            gas[2],
            gas[3],
            gas[4],
            gas[5],
            gas[6]
        );
    }
}

/*
 *  CALLBACKS FOR HANDLING FILTER RESULTS 
 */

fn player_joined_cb(result: Vec<Log>, player: Option<Address>) -> Option<bool> {
    for i in 0..result.len() {
        let data: &Vec<u8> = &result[i].data.0;
        let hash: H256 = H256::from(data.as_slice());
        let joined: Address = Address::from(hash);
        println!("Player joined: {:?}", joined);
        if player.unwrap() == joined {
            return Some(true);
        }
    }
    None
}

fn next_stage_cb(result: Vec<Log>, _: Option<Address>) -> Option<bool> {
    for i in 0..result.len() {
        let data: &Vec<u8> = &result[i].data.0;
        println!("New Stage: {:?}", U256::from(data.as_slice()).low_u64());
        return Some(true);
    }
    None
}

fn stage_result_cb(result: Vec<Log>, wanted: Option<Address>) -> Option<Vec<u8>> {
    for i in 0..result.len() {
        let hash_bytes: &[u8] = &result[i].data.0[0..32];
        let stage_hash: &[u8] = &result[i].data.0[96..142];
        let hash: H256 = H256::from(hash_bytes);
        let publisher: Address = Address::from(hash);
        println!("Player published results: {:?}", publisher);
        if publisher == wanted.unwrap() {
            return Some(stage_hash.to_vec());
        }
    }
    None
}

/*
    END OF CALLBACKS FOR FILTER RESULTS
 */