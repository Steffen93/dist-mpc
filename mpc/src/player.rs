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

#[macro_use]
extern crate serde_derive; 

extern crate json; 

mod protocol;
use self::protocol::*;
use protocol::{Transform, Verify};

#[cfg(feature = "snark")]
extern crate snark;
use snark::*;

use rand::{SeedableRng, Rng};
use bincode::SizeLimit::Infinite;
use bincode::rustc_serialize::{encode_into, encode, decode};
use sha3::{Digest, Keccak256};
use rustc_serialize::{Encodable, Decodable};

use serde_json::value::Value;

use web3::futures::Future;
use web3::contract::*;
use web3::contract::tokens::{Tokenize, Detokenize};
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
use std::path::Path;
use std::io::{Read, self};
use std::fs::{File};

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

fn get_current_state<T: Transport>(contract: &Contract<&T>, account: Address) -> u64 {
    let current_state: U256 = query_contract_default(contract, "currentState", (), account);
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

fn await_next_stage(filter: &BaseFilter<&Http, Log>, poll_interval: &Duration) {
    loop {
        let result = filter.poll().wait().expect("New Stage Filter should return result!").expect("Polling result should be valid!");
        for i in 0..result.len() {
            let data: &Vec<u8> = &result[i].data.0;
            println!("New Stage: {:?}", U256::from(data.as_slice()).low_u64());
            return;
        }
        thread::sleep(*poll_interval);
    }
}

fn await_stage_prepared(filter: &BaseFilter<&Http, Log>, poll_interval: &Duration) {
    loop {
        let result = filter.poll().wait().expect("Stage Prepared Filter should return result!").expect("Polling result should be valid!");
        for i in 0..result.len() {
            let data: &Vec<u8> = &result[i].data.0;
            println!("Stage {} prepared", U256::from(data.as_slice()).low_u64());
            return;
        }
        thread::sleep(*poll_interval);
    }
}

fn await_stage_result_published(filter: &BaseFilter<&Http, Log>, poll_interval: &Duration, wanted: Option<Address>) {
    if wanted.is_none() {
        println!("There is no player to wait for!");
        return
    }
    println!("Waiting for {:?} to commit first.", wanted.unwrap());
    loop {
        let result = filter.poll().wait().expect("Stage Result Published Filter should return result!").expect("Polling result should be valid!");
        for i in 0..result.len() {
            let data: &[u8] = &result[i].data.0[0..32];
            let publisher: Address = Address::from(data);
            println!("Player published results: {:?}", publisher);
            if publisher == wanted.unwrap() {
                return;
            }
        }
        thread::sleep(*poll_interval);
    }
}

fn await_player_joined(filter: &BaseFilter<&Http, Log>, poll_interval: &Duration, player: Address) {
    loop {
        let result = filter.poll().wait().expect("Player Joined Filter should return result!").expect("Polling result should be valid!");
        for i in 0..result.len() {
            let data: &Vec<u8> = &result[i].data.0;
            let joined: Address = Address::from(data.as_slice());
            println!("Player joined: {:?}", joined);
            if player == joined {
                return;
            }
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

fn upload_file_to_ipfs(path: &str, ipfs: &mut IPFS) -> IPFSAddResponse {
    let result = ipfs.add(path);
    println!("{:?}", String::from_utf8(result.clone()).unwrap());
    serde_json::from_slice(result.as_slice()).unwrap()
}

fn query_contract_default<T,P,R>(contract: &Contract<&T>, method: &str, params: P, account: Address) -> R where 
    P: Tokenize,
    R: Detokenize,
    T: Transport
{
    contract.query(method, params, account, Options::default(), BlockNumber::Latest).wait().expect(format!("Error querying contract method {:?}", method).as_str())
}

fn call_contract<T, P>(contract: &Contract<&T>, method: &str, params: P, account: Address) where 
    T: Transport,
    P: Tokenize
{
    let tokens = params.into_tokens();
    let gas = contract.estimate_gas(method, tokens.clone().as_slice(), account, Options::default()).wait().expect("Gas estimation should not fail!");
    println!("Gas estimation for {:?}: {:?}", method, gas.low_u64());
    contract.call(method, tokens.as_slice(), account, Options::with(|opt|{opt.gas = Some(U256::from(gas.low_u64()*3))})).wait().expect(format!("Error calling contract method {:?}", method).as_str());
}

fn download_stage<S, T>(contract: &Contract<&T>, account: Address, ipfs: &mut IPFS) -> S where 
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let stage_hash: Vec<u8> = query_contract_default(&contract, "getLatestTransformation", (), account);
    println!("Latest transformation hash: {:?}", String::from_utf8(stage_hash.clone()).unwrap());
    println!("Downloading stage from ipfs...");
    decode(&ipfs.cat(String::from_utf8(stage_hash).unwrap().as_str())).expect("Should match stage object!")
}

fn download_initial_stage<S, T>(contract: &Contract<&T>, account: Address, ipfs: &mut IPFS) -> S where 
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let stage_hash: Vec<u8> = query_contract_default(&contract, "getInitialStage", (), account);
    println!("Initial stage hash: {:?}", String::from_utf8(stage_hash.clone()).unwrap());
    println!("Downloading stage from ipfs...");
    decode(&ipfs.cat(String::from_utf8(stage_hash).unwrap().as_str())).expect("Should match stage object!")
}

fn transform_and_upload<S, T>(stage: &mut S, stage_init: &mut S, privkey: &PrivateKey, contract: &Contract<&T>, account: Address, file_name: &str, ipfs: &mut IPFS) where
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    println!("Transforming stage...");
    stage.transform(privkey);
    assert!(stage.is_well_formed(stage_init));
    let stage_transformed_ipfs = upload_to_ipfs(stage, file_name, ipfs);
    call_contract(contract, "publishStageResults", stage_transformed_ipfs.hash.into_bytes(), account);
}

fn upload_and_init<S, T>(stage: &mut S, contract: &Contract<&T>, account: Address, file_name: &str, ipfs: &mut IPFS) where
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let stage_ipfs = upload_to_ipfs(stage, file_name, ipfs);
    println!("Stage size: {} B", stage_ipfs.size);
    call_contract(contract, "setInitialStage", stage_ipfs.hash.into_bytes(), account);
}

fn deploy_contract<'a, T: Transport, P: AsRef<Path>>(web3: &Web3<&'a T>, path: P, account: Address, ipfs: &mut IPFS) -> Contract<&'a T>{
    let contract_build: &mut String = &mut String::new();
    File::open(path).expect("Error opening contract json file.").read_to_string(contract_build).expect("Should be readable.");
    let contract_build_json = json::parse(contract_build.as_str()).expect("Error parsing json!");
    let abi = &contract_build_json["abi"];
    let bytecode = &contract_build_json["bytecode"].dump();
    let len = bytecode.len()-1;
    let bytecode_hex: Vec<u8> = hex::decode(&bytecode[3..len]).unwrap();
    let cs_ipfs = upload_file_to_ipfs("r1cs", ipfs);
    
    Contract::deploy(web3.eth(), &abi.dump().into_bytes()).expect("Abi should be well-formed!")
    .options(Options::with(|opt|{opt.gas = Some(U256::from(3000000))}))
    .execute(bytecode_hex, cs_ipfs.hash.into_bytes(), account).expect("execute failed!").wait().expect("Error after wait!")
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

fn is_coordinator<T: Transport>(contract: &Contract<&T>, account: Address) -> bool {
    query_contract_default(contract, "isCoordinator", (), account)
}

fn get_players<T: Transport>(contract: &Contract<&T>, account: Address) -> Vec<Address> {
    let mut players: Vec<Address> = vec![];
    let number_of_players: u64 = query_contract_default(contract, "getNumberOfPlayers", (), account);
    for i in 0..number_of_players {
        let player: Address = query_contract_default(contract, "players", i, account);
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

fn fetch_all_commitments<T: Transport>(contract: &Contract<&T>, account: Address, players: Vec<Address>) -> Vec<Vec<u8>> {
    let mut all_commitments : Vec<Vec<u8>> = vec![];
    for player in players {
        let commitment: Vec<u8> = query_contract_default(contract, "getCommitment", player, account);
        all_commitments.push(commitment);
    }
    all_commitments
}

fn main() {
    // connect to web3
    println!("Connecting to Web3 instance...");
    let (_eloop, transport) = web3::transports::Http::new("http://localhost:8545").expect("Web3 cannot connect! (http://localhost:8545)");
    let web3 = Web3::new(&transport);
    println!("Successfully connected to web3 instance!");

    let mut ipfs = IPFS::new();
    ipfs.host("http://localhost", 5001);

    // Create filters:
    let next_stage_filter = create_filter(&web3, "NextStage(uint256)");
    let stage_prepared_filter = create_filter(&web3, "StagePrepared(uint256)");
    let player_joined_filter = create_filter(&web3, "PlayerJoined(address)");
    let await_stage_result_published_filter = create_filter(&web3, "StageResultPublished(address,bytes)");
    let duration = Duration::new(1, 0);

    let args: Vec<_> = env::args().collect();
    let contract: Contract<&Http>;
    let accounts: Vec<Address> = web3.eth().accounts().wait().unwrap();
    assert!(accounts.len() > 0);
    let mut default_account: Address = accounts[0];
    if args.len() > 1{
        let account_index: usize = args[1].parse().expect("Error reading account index from command line!");
        assert!(account_index < accounts.len());
        default_account = accounts[account_index];
    }
    println!("Account: {:?}", default_account);
    if args.len() > 2 {
        //get contract file(s)
        let contract_address: Address = args[2].parse().expect("Error reading the contract address from the command line!");

        //TODO: read given address as from_address from command line!
        contract = Contract::from_json(
            web3.eth(),
            contract_address,
            include_bytes!("../abi.json")
        ).expect("Error loading contract from json!");
        call_contract(&contract, "join", (), default_account);
        await_player_joined(&player_joined_filter, &duration, default_account);
    } else {
        contract = deploy_contract(&web3, "../blockchain/build/contracts/DistributedMPC.json", default_account, &mut ipfs);
    }
    println!("Contract address: {:?}", contract.address());

    let cs;
    if is_coordinator(&contract, default_account){
        println!("You are the coordinator. Reading r1cs.");
        // FIXME cs = CS::from_file();
        cs = CS::dummy();
    } else {
        println!("You are not the coordinator.");
        cs = CS::dummy();
    }

    // Start protocol
    prompt("Press [ENTER] when you're ready to begin the ceremony.");

    let mut chacha_rng = rand::chacha::ChaChaRng::from_seed(&get_entropy());

    //TODO: do all of this stuff later when start() has been called
    let privkey = PrivateKey::new(&mut chacha_rng);
    let pubkey = privkey.pubkey(&mut chacha_rng);
    let pubkey_encoded: Vec<u8> = encode(&pubkey, Infinite).unwrap();
    let commitment = pubkey.hash();
    let commitment_stringified = get_hex_string(&commitment);
    println!("Commitment: {}", commitment_stringified);
    assert_eq!(66, commitment_stringified.len());

    let mut stop = false;
    //end of Only Coordinator!
    let mut stage1: Stage1Contents;
    let mut stage2: Stage2Contents;
    let mut stage3: Stage3Contents;
    let mut players: Vec<Address> = vec![];
    let mut previous_player: Option<Address> = None;
    while !stop {
        match get_current_state(&contract, default_account) {
            0 => {
                if is_coordinator(&contract, default_account){
                    prompt("You are the coordinator. Press [ENTER] to start the protocol.");
                    call_contract(&contract, "start", (), default_account);
                } else {
                    println!("You are not the coordinator. The protocol will start as the coordinator decides.");
                }
                await_next_stage(&next_stage_filter, &duration);
                println!("Protocol Started!");
                players = get_players(&contract, default_account);
                previous_player = get_previous_player(players.clone(), default_account);
            },
            1 => {
                call_contract(&contract, "commit", to_bytes_fixed(&commitment.clone()), default_account);
                println!("Committed! Waiting for other players to commit...");
                await_next_stage(&next_stage_filter, &duration);
                println!("All players committed. Proceeding to next round.");
            },
            2 => {
                println!("Pubkey hex: {:?}", get_hex_string(&pubkey_encoded.clone()));
                call_contract(&contract, "revealCommitment", (pubkey_encoded.clone()), default_account);
                println!("Public Key revealed! Waiting for other players to reveal...");
                await_next_stage(&next_stage_filter, &duration);
                println!("All players revealed their commitments. Proceeding to next round.");
            },
            3 => {
                // TODO: fetch all commitments
                let mut all_commitments = fetch_all_commitments(&contract, default_account, players.clone());
                let hash_of_all_commitments = Digest512::from(&all_commitments).unwrap();
                println!("Creating nizks...");
                let nizks = pubkey.nizks(&mut chacha_rng, &privkey, &hash_of_all_commitments);
                println!("Nizks created.");
                assert!(nizks.is_valid(&pubkey, &hash_of_all_commitments));
                //TODO: check all nizks for validity!
                let nizks_encoded = encode(&nizks, Infinite).unwrap();
                println!("size of nizks: {} B", nizks_encoded.len());
                call_contract(&contract, "publishNizks", nizks_encoded, default_account);
                await_next_stage(&next_stage_filter, &duration);
            },
            4 => {
                if is_coordinator(&contract, default_account) {
                    println!("Creating stage...");
                    stage1 = Stage1Contents::new(&cs);
                    upload_and_init(&mut stage1, &contract, default_account, "stage1", &mut ipfs);
                    drop(stage1);
                }
                await_stage_prepared(&stage_prepared_filter, &duration);
                await_stage_result_published(&await_stage_result_published_filter, &duration, previous_player);
                let mut stage1_init: Stage1Contents = download_initial_stage(&contract, default_account, &mut ipfs);        //needed for transformation verification
                if previous_player.is_none(){
                    stage1 = stage1_init.clone();
                } else {
                    stage1 = download_stage(&contract, default_account, &mut ipfs);
                }
                transform_and_upload(&mut stage1, &mut stage1_init, &privkey, &contract, default_account, "stage1_transformed", &mut ipfs);
                drop(stage1);
                await_next_stage(&next_stage_filter, &duration);
            },
            5 => {
                if is_coordinator(&contract, default_account) {
                    stage1 = download_stage(&contract, default_account, &mut ipfs);
                    stage2 = Stage2Contents::new(&cs, &stage1);
                    drop(stage1);
                    upload_and_init(&mut stage2, &contract, default_account, "stage2", &mut ipfs);
                    drop(stage2);
                }
                await_stage_prepared(&stage_prepared_filter, &duration);
                await_stage_result_published(&await_stage_result_published_filter, &duration, previous_player);
                let mut stage2_init: Stage2Contents = download_initial_stage(&contract, default_account, &mut ipfs);        //needed for transformation verification
                if previous_player.is_none(){
                    stage2 = stage2_init.clone();
                } else {
                    stage2 = download_stage(&contract, default_account, &mut ipfs);
                }
                transform_and_upload(&mut stage2, &mut stage2_init, &privkey, &contract, default_account, "stage2_transformed", &mut ipfs);
                drop(stage2);
                await_next_stage(&next_stage_filter, &duration);
            },
            6 => {
                if is_coordinator(&contract, default_account) {
                    println!("Creating stage...");
                    stage2 = download_stage(&contract, default_account, &mut ipfs);
                    stage3 = Stage3Contents::new(&cs, &stage2);
                    drop(stage2);
                    upload_and_init(&mut stage3, &contract, default_account, "stage3", &mut ipfs);
                    drop(stage3);
                }
                await_stage_prepared(&stage_prepared_filter, &duration);
                await_stage_result_published(&await_stage_result_published_filter, &duration, previous_player);
                let mut stage3_init: Stage3Contents = download_initial_stage(&contract, default_account, &mut ipfs);        //needed for transformation verification
                if previous_player.is_none(){
                    stage3 = stage3_init.clone();
                } else {
                    stage3 = download_stage(&contract, default_account, &mut ipfs);
                }
                transform_and_upload(&mut stage3, &mut stage3_init, &privkey, &contract, default_account, "stage3_transformed", &mut ipfs);
                drop(stage3);
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

