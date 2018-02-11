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

#[macro_use]
extern crate serde_derive; 

extern crate json; 

mod protocol;
use self::protocol::*;
use protocol::{Transform, Verify};

mod blockchain;
use self::blockchain::*;

#[cfg(feature = "snark")]
extern crate snark;
use snark::*;

use spinner::SpinnerBuilder;

use rand::{SeedableRng, Rng};
use bincode::SizeLimit::Infinite;
use bincode::rustc_serialize::{encode_into, encode, decode};
use rustc_serialize::{Encodable, Decodable};

use serde_json::value::Value;

use web3::futures::Future;
use web3::contract::*;
use web3::contract::tokens::{Tokenize, Detokenize};
use web3::types::{Address, Log, U256, BlockNumber};
use web3::{Transport};
use web3::transports::Http;
use web3::Web3;

use ipfs_api::IPFS;

use std::env;
use std::time::Duration;
use std::fmt::Write;
use std::path::Path;
use std::io::{Write as FileWrite, Read, self};
use std::fs::{File};

pub const THREADS: usize = 8;
pub const DIRECTORY_PREFIX: &'static str = "/home/compute/";
pub const ASK_USER_TO_RECORD_HASHES: bool = true;
pub static mut TOTAL_GAS: u64 = 0;

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

fn upload_to_ipfs<T: Encodable>(obj: &T, name: &str, ipfs: &mut IPFS) -> IPFSAddResponse {
    let mut file = File::create(name).expect("Should work to create file.");
    encode_into(obj, &mut file, Infinite).unwrap();
    let result = ipfs.add(name);
    serde_json::from_slice(result.as_slice()).unwrap()
}

fn upload_file_to_ipfs(path: &str, ipfs: &mut IPFS) -> IPFSAddResponse {
    let result = ipfs.add(path);
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
    unsafe {
        TOTAL_GAS += gas.low_u64();
    }
    contract.call(method, tokens.as_slice(), account, Options::with(|opt|{opt.gas = Some(U256::from(gas.low_u64()*3))})).wait().expect(format!("Error calling contract method {:?}", method).as_str());
}

fn download_r1cs<T>(contract: &Contract<&T>, account: Address, ipfs: &mut IPFS) -> CS where 
    T: Transport
{
    let hash: Vec<u8> = query_contract_default(&contract, "getConstraintSystem", (), account);
    println!("R1CS hash: {:?}", String::from_utf8(hash.clone()).unwrap());
    println!("Downloading r1cs from ipfs...");
    let mut file = File::create("r1cs").unwrap();
    file.write_all(&ipfs.cat(String::from_utf8(hash).unwrap().as_str())).unwrap();
    // TODO: replace with cs from file
    CS::dummy()
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

fn download_final_stage<S, T>(contract: &Contract<&T>, stage: u64, account: Address, ipfs: &mut IPFS) -> S where 
    S: Transform + Verify + Clone + Encodable + Decodable,
    T: Transport
{
    let stage_hash: Vec<u8> = query_contract_default(&contract, "getLastTransformation", stage, account);
    println!("Final transformation hash of stage {:?}: {:?}", stage,  String::from_utf8(stage_hash.clone()).unwrap());
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
    let spinner = SpinnerBuilder::new("Transforming stage...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
    stage.transform(privkey);
    assert!(stage.is_well_formed(stage_init));
    spinner.message("Uploading transformation to ipfs...".into());
    let stage_transformed_ipfs = upload_to_ipfs(stage, file_name, ipfs);
    spinner.update("Publishing results on Ethereum...".into());
    call_contract(contract, "publishStageResults", stage_transformed_ipfs.hash.into_bytes(), account);
    spinner.close();
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
    let spinner = SpinnerBuilder::new("Deploying contract...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
    let contract_build: &mut String = &mut String::new();
    File::open(path).expect("Error opening contract json file.").read_to_string(contract_build).expect("Should be readable.");
    let contract_build_json = json::parse(contract_build.as_str()).expect("Error parsing json!");
    let abi = &contract_build_json["abi"];
    let bytecode = &contract_build_json["bytecode"].dump();
    let len = bytecode.len()-1;
    let bytecode_hex: Vec<u8> = hex::decode(&bytecode[3..len]).unwrap();
    let cs_ipfs = upload_file_to_ipfs("r1cs", ipfs);
    
    let contract = Contract::deploy(web3.eth(), &abi.dump().into_bytes()).expect("Abi should be well-formed!")
    .options(Options::with(|opt|{opt.gas = Some(U256::from(3000000))}))
    .execute(bytecode_hex, cs_ipfs.hash.into_bytes(), account).expect("execute failed!").wait().expect("Error after wait!");
    spinner.close();
    contract
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

    //connect to IPFS
    let mut ipfs = IPFS::new();
    ipfs.host("http://localhost", 5001);

    // Create filters:
    let filter_builder = EventFilterBuilder::new(&web3); 
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
    let mut player_joined_filter = filter_builder.create_filter("PlayerJoined(address)", "Waiting for player joining...".into(), player_joined_cb, Some(default_account));
    
    let mut cs = CS::dummy();
    if args.len() > 2 {
        let contract_address: Address = args[2].parse().expect("Error reading the contract address from the command line!");

        //TODO: read given address as from_address from command line!
        contract = Contract::from_json(
            web3.eth(),
            contract_address,
            include_bytes!("../abi.json")
        ).expect("Error loading contract from json!");
        call_contract(&contract, "join", (), default_account);
        player_joined_filter.await(&duration);
    } else {
        println!("You are the coordinator. Reading r1cs.");
        // FIXME cs = CS::from_file();
        cs = CS::dummy();
        contract = deploy_contract(&web3, "../blockchain/build/contracts/DistributedMPC.json", default_account, &mut ipfs);
    }
    println!("Contract address: {:?}", contract.address());

    let mut players: Vec<Address> = get_players(&contract, default_account);
    let mut previous_player: Option<Address> = get_previous_player(players.clone(), default_account);
    let mut next_stage_filter = filter_builder.create_filter("NextStage(uint256)", "Waiting for next stage to start...".into(), next_stage_cb, None);
    let mut stage_prepared_filter = filter_builder.create_filter("StagePrepared(uint256)","Waiting for next stage to be prepared by coordinator...".into(), stage_prepared_cb, None);
    let mut stage_result_published_filter = filter_builder.create_filter("StageResultPublished(address,bytes)", "Waiting for previous player to publish results...".into(), stage_result_cb, previous_player); 

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
    while !stop {
        match get_current_state(&contract, default_account) {
            0 => {
                if is_coordinator(&contract, default_account){
                    prompt("You are the coordinator. Press [ENTER] to start the protocol.");
                    call_contract(&contract, "start", (), default_account);
                } else {
                    println!("You are not the coordinator. The protocol will start as the coordinator decides.");
                }
                next_stage_filter.await(&duration);
                println!("Protocol Started!");
                players = get_players(&contract, default_account);
                previous_player = get_previous_player(players.clone(), default_account);
            },
            1 => {
                call_contract(&contract, "commit", to_bytes_fixed(&commitment.clone()), default_account);
                println!("Committed! Waiting for other players to commit...");
                next_stage_filter.await(&duration);
                println!("All players committed. Proceeding to next round.");
            },
            2 => {
                //println!("Pubkey hex: {:?}", get_hex_string(&pubkey_encoded.clone()));
                call_contract(&contract, "revealCommitment", (pubkey_encoded.clone()), default_account);
                println!("Public Key revealed! Waiting for other players to reveal...");
                next_stage_filter.await(&duration);
                println!("All players revealed their commitments. Proceeding to next round.");
            },
            3 => {
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
                next_stage_filter.await(&duration);
            },
            4 => {
                if is_coordinator(&contract, default_account) {
                    println!("Creating stage...");
                    stage1 = Stage1Contents::new(&cs);
                    upload_and_init(&mut stage1, &contract, default_account, "stage1", &mut ipfs);
                    drop(stage1);
                }
                stage_prepared_filter.await(&duration);
                if previous_player.is_some() {
                    stage_result_published_filter.await(&duration);                    
                }
                let mut stage1_init: Stage1Contents = download_initial_stage(&contract, default_account, &mut ipfs);        //needed for transformation verification
                if previous_player.is_none(){
                    stage1 = stage1_init.clone();
                } else {
                    stage1 = download_stage(&contract, default_account, &mut ipfs);
                }
                transform_and_upload(&mut stage1, &mut stage1_init, &privkey, &contract, default_account, "stage1_transformed", &mut ipfs);
                drop(stage1);
                next_stage_filter.await(&duration);
            },
            5 => {
                if is_coordinator(&contract, default_account) {
                    stage1 = download_stage(&contract, default_account, &mut ipfs);
                    stage2 = Stage2Contents::new(&cs, &stage1);
                    drop(stage1);
                    upload_and_init(&mut stage2, &contract, default_account, "stage2", &mut ipfs);
                    drop(stage2);
                }
                stage_prepared_filter.await(&duration);
                if previous_player.is_some() {
                    stage_result_published_filter.await(&duration);                    
                }
                let mut stage2_init: Stage2Contents = download_initial_stage(&contract, default_account, &mut ipfs);        //needed for transformation verification
                if previous_player.is_none(){
                    stage2 = stage2_init.clone();
                } else {
                    stage2 = download_stage(&contract, default_account, &mut ipfs);
                }
                transform_and_upload(&mut stage2, &mut stage2_init, &privkey, &contract, default_account, "stage2_transformed", &mut ipfs);
                drop(stage2);
                next_stage_filter.await(&duration);
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
                stage_prepared_filter.await(&duration);
                if previous_player.is_some() {
                    stage_result_published_filter.await(&duration);                    
                }
                let mut stage3_init: Stage3Contents = download_initial_stage(&contract, default_account, &mut ipfs);        //needed for transformation verification
                if previous_player.is_none(){
                    stage3 = stage3_init.clone();
                } else {
                    stage3 = download_stage(&contract, default_account, &mut ipfs);
                }
                transform_and_upload(&mut stage3, &mut stage3_init, &privkey, &contract, default_account, "stage3_transformed", &mut ipfs);
                drop(stage3);
                next_stage_filter.await(&duration);
            },
            7 => {
                let spinner = SpinnerBuilder::new("Protocol finished! Downloading final stages...".into()).spinner(spinner::DANCING_KIRBY.to_vec()).step(Duration::from_millis(500)).start();
                let cs: CS = download_r1cs(&contract, default_account, &mut ipfs);
                spinner.message("R1CS complete.".into());
                stage1 = download_final_stage(&contract, 0, default_account, &mut ipfs);
                spinner.message("Stage1 complete.".into());
                stage2 = download_final_stage(&contract, 1, default_account, &mut ipfs);
                spinner.message("Stage2 complete.".into());
                stage3 = download_final_stage(&contract, 2, default_account, &mut ipfs);
                spinner.message("Stage3 complete.".into());
                // Download r1cs, stage1, stage2, stage3 from ipfs
                let kp = keypair(&cs, &stage1, &stage2, &stage3);
                kp.write_to_disk();
                spinner.close();
                unsafe {
                    println!("Total gas estimation: {}", TOTAL_GAS);
                }
                stop = true;
            }
            _ => {
                stop = true;
            }
        }
    }
}

