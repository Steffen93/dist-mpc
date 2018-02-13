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
extern crate ethabi;
extern crate hex;
extern crate spinner;
extern crate json;

#[macro_use]
extern crate clap;
use clap::{App};

#[macro_use]
extern crate serde_derive; 
use serde_json::value::Value;

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

use spinner::SpinnerBuilder;

use bincode::SizeLimit::Infinite;
use bincode::rustc_serialize::{encode};
use rustc_serialize::{Encodable, Decodable};

use web3::contract::tokens::Tokenize;
use web3::types::{Address, Log, U256};
use web3::{Transport, Web3};
use web3::transports::Http;

use rand::{SeedableRng, Rng};

use std::time::Duration;
use std::fmt::Write;
use std::io::{self};
use std::fs::File;

pub const THREADS: usize = 8;
pub const DIRECTORY_PREFIX: &'static str = "/home/compute/";
pub const ASK_USER_TO_RECORD_HASHES: bool = true;

#[derive(Deserialize, Debug)]
struct ContractJson {
    abi: Vec<Value>,
    bytecode: String
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

fn get_current_state<T: Transport>(contract: &ContractWrapper<T>) -> u64 {
    let current_state: U256 = contract.query("currentState", ());
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


fn download_r1cs<T>(contract: &ContractWrapper<T>, ipfs: &mut IPFSWrapper) -> CS where 
    T: Transport
{
    let hash: Vec<u8> = contract.query("getConstraintSystem", ());
    println!("R1CS hash: {:?}", String::from_utf8(hash.clone()).unwrap());
    println!("Downloading r1cs from ipfs...");
    ipfs.download_cs(String::from_utf8(hash).unwrap().as_str())
}
fn download_stage<P, S, T>(contract: &ContractWrapper<T>, method: &str, params: P, ipfs: &mut IPFSWrapper) -> S where 
    P: Tokenize,
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let stage_hash: Vec<u8> = contract.query(method, params);
    println!("Downloading stage from IPFS (hash: {:?})", String::from_utf8(stage_hash.clone()).unwrap());
    ipfs.download_stage(String::from_utf8(stage_hash).unwrap().as_str())
}

fn transform_and_upload<S, T>(stage: &mut S, stage_init: &mut S, privkey: &PrivateKey, contract: &ContractWrapper<T>, file_name: &str, ipfs: &mut IPFSWrapper) where
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let spinner = SpinnerBuilder::new("Transforming stage...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
    stage.transform(privkey);
    assert!(stage.is_well_formed(stage_init));
    spinner.message("Uploading transformation to ipfs...".into());
    let stage_transformed_ipfs = ipfs.upload_object(stage, file_name);
    spinner.update("Publishing results on Ethereum...".into());
    contract.call("publishStageResults", stage_transformed_ipfs.hash.into_bytes());
    spinner.close();
}

fn upload_and_init<S, T>(stage: &mut S, contract: &ContractWrapper<T>, file_name: &str, ipfs: &mut IPFSWrapper) where
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let stage_ipfs = ipfs.upload_object(stage, file_name);
    println!("Stage size: {} B", stage_ipfs.size);
    contract.call("setInitialStage", stage_ipfs.hash.into_bytes());
}

fn prompt(s: &str) -> String {
    loop {
        let mut input = String::new();
        //reset();
        println!("{}", s);
        println!("\x07");

        if io::stdin().read_line(&mut input).is_ok() {
            println!("Please wait...");
            return (&input[0..input.len()-1]).into();
        }
    }
}

fn is_coordinator<T: Transport>(contract: &ContractWrapper<T>) -> bool {
    contract.query("isCoordinator", ())
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

fn main() {
    let yaml = load_yaml!("../cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let account_index = matches.value_of("account");
    let contract_address = matches.value_of("contract");

    println!("Connecting to Web3 and IPFS...");
    let (_eloop, transport) = Http::new("http://localhost:8545").expect("Error connecting to web3 instance!");
    let mut manager: Manager<Http> = Manager::new(Web3::new(transport), "http://localhost", 5001);
    println!("Successfully connected!");

    let web3: Web3<Http> = manager.web3.clone();
    let mut ipfsw: IPFSWrapper = IPFSWrapper::new("http://localhost", 5001);

    //Filter poll interval
    let default_account: Address = manager.init_account(account_index);
    println!("Account: {:?}", default_account);    
    
    let cs = CS::dummy();
    let contract = manager.init_contract(account_index, contract_address);

    let filter_builder = EventFilterBuilder::new(web3.clone()); 
    let poll_interval = Duration::new(1, 0);
    let mut player_joined_filter = filter_builder.create_filter("PlayerJoined(address)", "Waiting for player joining...".into(), player_joined_cb, Some(default_account));
    let mut next_stage_filter = filter_builder.create_filter("NextStage(uint256)", "Waiting for next stage to start...".into(), next_stage_cb, None);
    let mut stage_prepared_filter = filter_builder.create_filter("StagePrepared(uint256)","Waiting for next stage to be prepared by coordinator...".into(), stage_prepared_cb, None);
    
    // IF CURRENT ACCOUNT IS NOT A PLAYER, JOIN!
    let is_player: bool = contract.query("isPlayer", ());
    if !is_player {
        contract.call("join", ());
        player_joined_filter.await(&poll_interval);    
    }

    let mut players: Vec<Address> = get_players(&contract);
    let mut previous_player: Option<Address> = get_previous_player(players.clone(), default_account);
    let mut stage_result_published_filter = filter_builder.create_filter("StageResultPublished(address,bytes)", "Waiting for previous player to publish results...".into(), stage_result_cb, previous_player); 
    // IF COORDINATOR: then the R1CS will have been uploaded to ipfs during deployment
    // FIXME cs = CS::from_file();
    //cs = CS::dummy();

    // Start protocol
    prompt("Press [ENTER] when you're ready to begin the ceremony.");

    let mut chacha_rng = rand::chacha::ChaChaRng::from_seed(&get_entropy());

    //TODO: do all of this stuff later when start() has been called
    let privkey = PrivateKey::new(&mut chacha_rng);
    let pubkey = privkey.pubkey(&mut chacha_rng);
    let pubkey_encoded: Vec<u8> = encode(&pubkey, Infinite).unwrap();
    let commitment = pubkey.hash();

    let mut stop = false;
    //end of Only Coordinator!
    let mut stage1: Stage1Contents;
    let mut stage2: Stage2Contents;
    let mut stage3: Stage3Contents;
    while !stop {
        match get_current_state(&contract) {
            0 => {
                if is_coordinator(&contract){
                    prompt("You are the coordinator. Press [ENTER] to start the protocol.");
                    contract.call("start", ());
                } else {
                    println!("You are not the coordinator. The protocol will start as the coordinator decides.");
                }
                next_stage_filter.await(&poll_interval);
                println!("Protocol Started!");
                players = get_players(&contract);
                previous_player = get_previous_player(players.clone(), default_account);
            },
            1 => { 
                contract.call("commit", to_bytes_fixed(&commitment.clone()));
                println!("Committed! Waiting for other players to commit...");
                next_stage_filter.await(&poll_interval);
                println!("All players committed. Proceeding to next round.");
            },
            2 => {
                //println!("Pubkey hex: {:?}", get_hex_string(&pubkey_encoded.clone()));
                contract.call("revealCommitment", pubkey_encoded.clone());
                println!("Public Key revealed! Waiting for other players to reveal...");
                next_stage_filter.await(&poll_interval);
                println!("All players revealed their commitments. Proceeding to next round.");
            },
            3 => {
                let mut all_commitments = fetch_all_commitments(&contract, players.clone());
                let hash_of_all_commitments = Digest512::from(&all_commitments).unwrap();
                println!("Creating nizks...");
                let nizks = pubkey.nizks(&mut chacha_rng, &privkey, &hash_of_all_commitments);
                println!("Nizks created.");
                assert!(nizks.is_valid(&pubkey, &hash_of_all_commitments));
                //TODO: check all nizks for validity!
                let nizks_encoded = encode(&nizks, Infinite).unwrap();
                println!("size of nizks: {} B", nizks_encoded.len());
                contract.call("publishNizks", nizks_encoded);
                next_stage_filter.await(&poll_interval);
            },
            4 => {
                if is_coordinator(&contract) {
                    println!("Creating stage...");
                    stage1 = Stage1Contents::new(&cs);
                    upload_and_init(&mut stage1, &contract, "stage1", &mut ipfsw);
                    drop(stage1);
                }
                stage_prepared_filter.await(&poll_interval);
                if previous_player.is_some() {
                    stage_result_published_filter.await(&poll_interval);                    
                }
                let mut stage1_init: Stage1Contents = download_stage(&contract, "getInitialStage", (), &mut ipfsw);        //needed for transformation verification
                if previous_player.is_none(){
                    stage1 = stage1_init.clone();
                } else {
                    stage1 = download_stage(&contract, "getLatestTransformation", (), &mut ipfsw);
                }
                transform_and_upload(&mut stage1, &mut stage1_init, &privkey, &contract, "stage1_transformed", &mut ipfsw);
                drop(stage1);
                next_stage_filter.await(&poll_interval);
            },
            5 => {
                if is_coordinator(&contract) {
                    stage1 = download_stage(&contract, "getLatestTransformation", (), &mut ipfsw);
                    stage2 = Stage2Contents::new(&cs, &stage1);
                    drop(stage1);
                    upload_and_init(&mut stage2, &contract, "stage2", &mut ipfsw);
                    drop(stage2);
                }
                stage_prepared_filter.await(&poll_interval);
                if previous_player.is_some() {
                    stage_result_published_filter.await(&poll_interval);                    
                }
                let mut stage2_init: Stage2Contents = download_stage(&contract, "getInitialStage", (), &mut ipfsw);        //needed for transformation verification
                if previous_player.is_none(){
                    stage2 = stage2_init.clone();
                } else {
                    stage2 = download_stage(&contract, "getLatestTransformation", (), &mut ipfsw);
                }
                transform_and_upload(&mut stage2, &mut stage2_init, &privkey, &contract, "stage2_transformed", &mut ipfsw);
                drop(stage2);
                next_stage_filter.await(&poll_interval);
            },
            6 => {
                if is_coordinator(&contract) {
                    println!("Creating stage...");
                    stage2 = download_stage(&contract, "getLatestTransformation", (), &mut ipfsw);
                    stage3 = Stage3Contents::new(&cs, &stage2);
                    drop(stage2); 
                    upload_and_init(&mut stage3, &contract, "stage3", &mut ipfsw);
                    drop(stage3);
                }
                stage_prepared_filter.await(&poll_interval);
                if previous_player.is_some() {
                    stage_result_published_filter.await(&poll_interval);                    
                }
                let mut stage3_init: Stage3Contents = download_stage(&contract, "getInitialStage", (), &mut ipfsw);        //needed for transformation verification
                if previous_player.is_none(){
                    stage3 = stage3_init.clone();
                } else { 
                    stage3 = download_stage(&contract, "getLatestTransformation", (), &mut ipfsw);
                }
                transform_and_upload(&mut stage3, &mut stage3_init, &privkey, &contract, "stage3_transformed", &mut ipfsw);
                drop(stage3);
                next_stage_filter.await(&poll_interval);
            },
            7 => {
                let spinner = SpinnerBuilder::new("Protocol finished! Downloading final stages...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
                let cs: CS = download_r1cs(&contract, &mut ipfsw);
                spinner.message("R1CS complete.".into());
                stage1 = download_stage(&contract, "getLastTransformation", 0, &mut ipfsw);
                spinner.message("Stage1 complete.".into());
                stage2 = download_stage(&contract, "getLastTransformation", 1, &mut ipfsw);
                spinner.message("Stage2 complete.".into());
                stage3 = download_stage(&contract, "getLastTransformation", 2, &mut ipfsw);
                spinner.message("Stage3 complete.".into());
                // Download r1cs, stage1, stage2, stage3 from ipfs
                let kp = keypair(&cs, &stage1, &stage2, &stage3);
                kp.write_to_disk();
                spinner.close();
                stop = true;
            }
            _ => {
                stop = true;
            }
        }
    }
}
/*
 *  CALLBACKS FOR HANDLING FILTER RESULTS 
 */
fn next_stage_cb(result: Vec<Log>, _: Option<Address>) -> bool {
    for i in 0..result.len() {
        let data: &Vec<u8> = &result[i].data.0;
        println!("New Stage: {:?}", U256::from(data.as_slice()).low_u64());
        return true;
    }
    false
}

fn stage_prepared_cb(result: Vec<Log>, _: Option<Address>) -> bool {
    for i in 0..result.len() {
            let data: &Vec<u8> = &result[i].data.0;
            println!("Stage {} prepared", U256::from(data.as_slice()).low_u64());
            return true;
        }
        false
}

fn player_joined_cb(result: Vec<Log>, player: Option<Address>) -> bool {
    for i in 0..result.len() {
        let data: &Vec<u8> = &result[i].data.0;
        let joined: Address = Address::from(data.as_slice());
        println!("Player joined: {:?}", joined);
        if player.unwrap() == joined {
            return true;
        }
    }
    false
}

fn stage_result_cb(result: Vec<Log>, wanted: Option<Address>) -> bool {
    for i in 0..result.len() {
        let data: &[u8] = &result[i].data.0[0..32];
        let publisher: Address = Address::from(data);
        println!("Player published results: {:?}", publisher);
        if publisher == wanted.unwrap() {
            return true;
        }
    }
    false
}

/*
    END OF CALLBACKS FOR FILTER RESULTS
 */