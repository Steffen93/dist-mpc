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
extern crate ethabi; 
extern crate serde;

extern crate log;
extern crate env_logger;
extern crate time;
extern crate ansi_term;

use web3::futures::Future;
use web3::contract::*;
use web3::types::{Address, Filter, FilterBuilder, U256, H256, BlockNumber};
use web3::{Transport};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Read};
use std::str::FromStr;
use std::time::Duration;
use std::thread;

/*
impl ConnectionHandler {


    fn run(&self, new_peers: Receiver<[u8; 8]>)
    {
        use std::fs::File;

        info!("Loading R1CS from disk and performing QAP reduction...");

        let cs = {
            if USE_DUMMY_CS {
                CS::dummy()
            } else {
                CS::from_file()
            }
        };

        info!("Creating transcript file...");
        let mut transcript = File::create("transcript").unwrap();
        encode_into(&PLAYERS, &mut transcript, Infinite).unwrap();

        info!("Waiting for players to connect...");

        let mut peers = vec![];
        let mut commitments: Vec<Digest256> = vec![];
        for peerid in new_peers.into_iter().take(PLAYERS) {
            info!("Initializing new player (peerid={})", peerid.to_hex());
            info!("Asking for commitment to PublicKey (peerid={})", peerid.to_hex());
            let comm: Digest256 = self.read(&peerid);
            info!("PublicKey Commitment received (peerid={})", peerid.to_hex());

            info!("Writing commitment to transcript");
            encode_into(&comm, &mut transcript, Infinite).unwrap();

            commitments.push(comm);
            peers.push(peerid);
        }

        // The remote end should never hang up, so this should always be `PLAYERS`.
        assert_eq!(peers.len(), PLAYERS);

        // Hash of all the commitments.
        let hash_of_commitments = Digest512::from(&commitments).unwrap();

        info!("All players are ready");

        // Hash of the last message
        let mut last_message_hash = Digest256::from(&commitments).unwrap();

        info!("Initializing stage1 with constraint system");

        let mut stage1 = Stage1Contents::new(&cs);
        for (comm, peerid) in commitments.iter().zip(peers.iter()) {
            info!("Sending stage1 to peerid={}", peerid.to_hex());

            self.write(peerid, &hash_of_commitments);
            self.write(peerid, &stage1);
            self.write(peerid, &last_message_hash);

            info!("Receiving public key from peerid={}", peerid.to_hex());
            let pubkey = self.read::<PublicKey>(peerid);

            info!("Receiving nizks from peerid={}", peerid.to_hex());
            let nizks = self.read::<PublicKeyNizks>(peerid);

            if pubkey.hash() != *comm {
                error!("Peer did not properly commit to their public key (peerid={})", peerid.to_hex());
                panic!("cannot recover.");
            }

            if !nizks.is_valid(&pubkey, &hash_of_commitments) {
                error!("Peer did not provide proof that they possess the secrets! (peerid={})", peerid.to_hex());
                panic!("cannot recover.");
            }

            info!("Receiving stage1 transformation from peerid={}", peerid.to_hex());
            let new_stage1 = self.read::<Stage1Contents>(peerid);

            let ihash = self.read::<Digest256>(peerid);

            if !new_stage1.is_well_formed(&stage1) {
                error!("Peer did not perform valid stage1 transformation (peerid={})", peerid.to_hex());
                panic!("cannot recover.");
            } else {
                info!("Writing `PublicKey` to transcript");
                encode_into(&pubkey, &mut transcript, Infinite).unwrap();
                info!("Writing `PublicKeyNizks` to transcript");
                encode_into(&nizks, &mut transcript, Infinite).unwrap();
                info!("Writing new stage1 to transcript");
                encode_into(&new_stage1, &mut transcript, Infinite).unwrap();

                encode_into(&ihash, &mut transcript, Infinite).unwrap();

                last_message_hash = digest256_from_parts!(
                    pubkey, nizks, new_stage1, ihash
                );

                stage1 = new_stage1;
            }
        }

        info!("Initializing stage2 with constraint system and stage1");

        let mut stage2 = Stage2Contents::new(&cs, &stage1);
        for peerid in peers.iter() {
            info!("Sending stage2 to peerid={}", peerid.to_hex());

            self.write(peerid, &stage2);
            self.write(peerid, &last_message_hash);

            info!("Receiving stage2 transformation from peerid={}", peerid.to_hex());

            let new_stage2 = self.read::<Stage2Contents>(peerid);
            let ihash = self.read::<Digest256>(peerid);

            if !new_stage2.is_well_formed(&stage2) {
                error!("Peer did not perform valid stage2 transformation (peerid={})", peerid.to_hex());
                panic!("cannot recover.");
            } else {
                info!("Writing new stage2 to transcript");
                encode_into(&new_stage2, &mut transcript, Infinite).unwrap();
                encode_into(&ihash, &mut transcript, Infinite).unwrap();

                last_message_hash = digest256_from_parts!(
                    new_stage2, ihash
                );

                stage2 = new_stage2;
            }
        }

        info!("Initializing stage3 with constraint system and stage2");

        let mut stage3 = Stage3Contents::new(&cs, &stage2);
        for peerid in peers.iter() {
            info!("Sending stage3 to peerid={}", peerid.to_hex());

            self.write(peerid, &stage3);
            self.write(peerid, &last_message_hash);

            info!("Receiving stage3 transformation from peerid={}", peerid.to_hex());

            let new_stage3 = self.read::<Stage3Contents>(peerid);
            let ihash = self.read::<Digest256>(peerid);

            info!("Verifying transformation of stage3 from peerid={}", peerid.to_hex());

            if !new_stage3.is_well_formed(&stage3) {
                error!("Peer did not perform valid stage3 transformation (peerid={})", peerid.to_hex());
                panic!("cannot recover.");
            } else {
                info!("Writing new stage3 to transcript");
                encode_into(&new_stage3, &mut transcript, Infinite).unwrap();
                encode_into(&ihash, &mut transcript, Infinite).unwrap();

                last_message_hash = digest256_from_parts!(
                    new_stage3, ihash
                );

                stage3 = new_stage3;
            }
        }

        info!("MPC complete, flushing transcript to disk.");

        transcript.flush().unwrap();

        info!("Transcript flushed to disk.");
    }

    fn accept(&self, peerid: [u8; 8], mut stream: TcpStream, remote_msgid: u8) {
        use std::collections::hash_map::Entry::{Occupied, Vacant};

        fn send_msgid(stream: &mut TcpStream, msgid: u8) {
            let _ = stream.write_all(&[msgid]);
            let _ = stream.flush();
        }

        let mut peers = self.peers.lock().unwrap();

        match peers.entry(peerid) {
            Occupied(mut already) => {
                if already.get().is_none() {
                    warn!("Ignoring duplicate connection attempt (peerid={})", peerid.to_hex());
                } else {
                    let our_msgid = match already.get() {
                        &Some((_, our_msgid, _)) => our_msgid,
                        _ => unreachable!()
                    };
                    send_msgid(&mut stream, our_msgid);
                    already.insert(Some((stream, our_msgid, remote_msgid)));
                }
            },
            Vacant(vacant) => {
                match self.notifier.send(peerid) {
                    Ok(_) => {
                        info!("Accepted new connection (peerid={})", peerid.to_hex());
                        send_msgid(&mut stream, 0);
                        vacant.insert(Some((stream, 0, remote_msgid)));
                    },
                    Err(_) => {
                        warn!("Rejecting connection from peerid={}, no longer accepting new players.", peerid.to_hex());
                    }
                }
            }
        }
    }
}
*/

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

fn main() {
    //get contract file(s)
    let contract_address: Address = "0xd0278c39d3024b5975f1e9c3cc292eab37675950".parse().unwrap();
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
    // 
    let mut filterBuilder: FilterBuilder = FilterBuilder::default();
    filterBuilder = filterBuilder.topics(Some(vec![H256::from_str("0xf2f13d712bddc038fd1341d24bad63155a3e68fb5b398cb8f170cd736c277505").unwrap()]), None, None, None);
    let filter: Filter = filterBuilder.build();
    let createFilter = web3.eth_filter().create_logs_filter(filter);
    let baseFilter = createFilter.wait().expect("Filter should be registerable!");

    //let mut currentState: u64 = get_current_state(&contract, &default_account);
    //println!("Current State: {:?}", currentState);
    //start protocol:

    let duration = Duration::new(1, 0);
    let mut commitment = String::new();
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
                println!("Please enter commitment: ");
                std::io::stdin().read_line(&mut commitment).expect("Failed to read line");
                commitment.pop(); // remove trailing line break
                let hashed_commitment: H256 = web3.web3().sha3(commitment.clone().into()).wait().unwrap();
                println!("Hashed commitment: {:?}", hashed_commitment);
                contract.call("commit", format!("{}",hashed_commitment), default_account, Options::default()).wait().expect("Commit failed!");
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
                contract.call("publishPlayerData", (String::from("Thisismynizks"), commitment.clone()), default_account, Options::default()).wait().expect("Error publishing commitment origin!");
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
        


        
    // assume that we start from scratch, later: get current state
}
