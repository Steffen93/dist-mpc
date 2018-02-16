#![allow(non_snake_case, dead_code)]

extern crate bn;
extern crate rand;
extern crate snark;
extern crate crossbeam;
extern crate rustc_serialize;
extern crate blake2_rfc;
extern crate bincode;
extern crate byteorder;
extern crate web3;
extern crate hex;
extern crate json;
extern crate serde_json;
extern crate ipfs_api;
extern crate sha3;
extern crate spinner;

#[macro_use]
extern crate clap;
use clap::{App};

#[macro_use]
extern crate serde_derive; 

#[macro_use]
mod protocol;

mod consts;
use self::consts::*;

mod manager;
use self::manager::*;

mod dist_files;
use self::dist_files::*;

mod blockchain;
use blockchain::*;

use bincode::rustc_serialize::{decode};

use protocol::*;
use snark::*;
use std::env::var;
use std::time::Duration;
use spinner::SpinnerBuilder;

use web3::{Transport, Web3};
use web3::transports::Http;
use web3::types::{Address};

pub struct PlayerResult {
    player: Address,
    pubkey: PublicKey,
    nizks: PublicKeyNizks,
    stage1: Stage1Contents,
    stage2: Stage2Contents,
    stage3: Stage3Contents    
}


fn download_r1cs<T>(contract: &ContractWrapper<T>, ipfs: &mut IPFSWrapper) -> CS where 
    T: Transport
{
    let spinner = SpinnerBuilder::new("Querying constraint system hash from Ethereum...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
    let hash: Vec<u8> = contract.query("getConstraintSystem", ());
    spinner.message(format!("Downloading constraint system from ipfs (hash: {:?})...", String::from_utf8(hash.clone()).unwrap()));
    let cs = ipfs.download_cs(String::from_utf8(hash).unwrap().as_str());
    spinner.close();
    cs
}

fn main() {
    let host_opt = var(HOST_ENV_KEY);
    let mut host = String::from(DEFAULT_HOST);
    if host_opt.is_ok() {
            host = host_opt.unwrap();
            println!("Using host from environment variable: {:?}", host);
    }

    let yaml = load_yaml!("../verifier.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let contract_address = matches.value_of("contract");

    println!("Initializing Web3 and IPFS...");
    let (_eloop, transport) = Http::new(format!("http://{}:8545", host).as_str()).expect("Error connecting to web3 instance!");
    let manager: Manager<Http> = Manager::new(Web3::new(transport), format!("http://{}", host).as_str(), 5001);
    let mut ipfs: IPFSWrapper = IPFSWrapper::new(format!("http://{}", host).as_str(), 5001);
    println!("Successfully initialized.");

    let contract = manager.init_contract(None, contract_address);

    let cs = download_r1cs(&contract, &mut ipfs);

    let mut commitments = vec![];
    let number_of_players: u64 = contract.query("getNumberOfPlayers", ());
    let mut players: Vec<PlayerResult> = vec![];
    let spinner = SpinnerBuilder::new("Collecting player information from Ethereum and IPFS...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();            
    for i in 0..number_of_players { 
        let player: Address = contract.query("players", i);
        let commitment: [u8; 32] = contract.query("getCommitment", player);
        commitments.push(commitment);
        let publickey: Vec<u8> = contract.query("getPublicKey", i);
        let nizks: Vec<u8> = contract.query("getNizks", i);
        let stage1_hash: Vec<u8> = contract.query("getTransformation", (0, i));
        let stage2_hash: Vec<u8> = contract.query("getTransformation", (1, i));
        let stage3_hash: Vec<u8> = contract.query("getTransformation", (2, i));
        players.push(PlayerResult {
            player: player,
            pubkey: decode(&publickey).expect("Error decoding public key to object"),
            nizks: decode(&nizks).expect("Error decoding nizks to object"),
            stage1: ipfs.download_stage(String::from_utf8(stage1_hash).expect("Error decoding stage 1 to object").as_str()),
            stage2: ipfs.download_stage(String::from_utf8(stage2_hash).expect("Error decoding stage 2 to object").as_str()),
            stage3: ipfs.download_stage(String::from_utf8(stage3_hash).expect("Error decoding stage 3 to object").as_str())
        });
    }
    spinner.close();

    // Hash of all the commitments.
    let hash_of_commitments = Digest512::from(&commitments).unwrap();

    // Hash of the last message
    let mut stage1 = &Stage1Contents::new(&cs);
    for i in 0..players.len() {
        let pubkey: &PublicKey = &players[i].pubkey;
        if pubkey.hash() != commitments[i] {
            panic!("\u{274c} Invalid commitment from player {}", i);
        }
        println!("\u{2714} Commitment of player {} matches public key hash", i);
        let nizks: &PublicKeyNizks = &players[i].nizks;
        if !nizks.is_valid(&pubkey, &hash_of_commitments) {
            panic!("\u{274c} Invalid nizks from player {}", i);
        }
        println!("\u{2714} Nizks of player {} is valid", i);
        let new_stage: &Stage1Contents = &players[i].stage1;
        if !new_stage.verify_transform(&stage1, &pubkey) {
            panic!("\u{274c} Invalid stage1 transformation from player {}", i);
        }
        println!("\u{2714} Stage 1 has been transformed correctly by player {}", i);
        stage1 = new_stage;
    }

    let mut stage2 = &Stage2Contents::new(&cs, stage1);
    for i in 0..players.len() {
        let new_stage: &Stage2Contents = &players[i].stage2;
        if !new_stage.verify_transform(stage2, &players[i].pubkey) {
            panic!("\u{274c} Invalid stage2 transformation from player {}", i);
        }
        println!("\u{2714} Stage 2 has been transformed correctly by player {}", i);
        stage2 = new_stage;
    }

    let mut stage3 = &Stage3Contents::new(&cs, stage2);
    for i in 0..players.len() {
        let new_stage: &Stage3Contents = &players[i].stage3;
        if !new_stage.verify_transform(stage3, &players[i].pubkey) {
            panic!("\u{274c} Invalid stage3 transformation from player {}", i);
        }
        println!("\u{2714} Stage 3 has been transformed correctly by player {}", i);
        stage3 = new_stage;
    }

    let kp = keypair(&cs, stage1, stage2, stage3);
    kp.write_to_disk();
    println!("\u{2714} Verification successful. Wrote keypair to disk as (pk, vk).");
}
