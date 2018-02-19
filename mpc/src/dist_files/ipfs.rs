use bincode::rustc_serialize::{encode_into, decode};
use bincode::SizeLimit::Infinite;
use ipfs_api::IPFS;
use protocol::{Transform, Verify};
use rustc_serialize::{Encodable, Decodable};
use serde_json;
use snark::CS;
use std::fs::File;
use std::io::Write;

pub struct IPFSWrapper {
    ipfs: IPFS
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct IPFSAddResponse {
    pub name: String,
    pub hash: String,
    pub size: String
}

impl IPFSWrapper {
    pub fn new(host: &str, port: u16) -> Self {
        let mut _ipfs = IPFS::new();
        _ipfs.host(host, port);
        IPFSWrapper{
            ipfs: _ipfs
        }
    }

    pub fn download_stage<S>(&mut self, hash: &str) -> S where
    S: Encodable + Decodable + Transform + Verify + Clone
    {
        decode(&self.ipfs.cat(hash)).expect("Should be decodable to a stage object!")
    }

    pub fn download_cs(&mut self, hash: &str) -> CS {
        let mut file = File::create("r1cs").expect("Unexpected Error in IPFS Wrapper!");
        file.write_all(&self.ipfs.cat(hash)).expect("Unexpected Error in IPFS Wrapper!");
        // TODO: replace with cs from file
        CS::from_file()
    }

    pub fn upload_object<T>(&mut self, obj: &T, name: &str) -> IPFSAddResponse where
    T: Encodable
    {
        let mut file = File::create(name).expect("Should work to create file.");
        encode_into(obj, &mut file, Infinite).expect("Unexpected Error in IPFS Wrapper!");
        let result = self.ipfs.add(name);
        serde_json::from_slice(result.as_slice()).expect("Unexpected Error in IPFS Wrapper!")
    }

    pub fn upload_file(&mut self, path: &str) -> IPFSAddResponse {
        let result = self.ipfs.add(path);
        serde_json::from_slice(result.as_slice()).expect("Unexpected Error in IPFS Wrapper!")
    }
}