use bn::Fr;

use rustc_serialize::{Encodable, Encoder, Decodable, Decoder};
use bincode::SizeLimit::Infinite;
use bincode::rustc_serialize::encode;
use blake2_rfc::blake2b::blake2b;

macro_rules! digest_impl {
    ($name:ident, $bytes:expr, $hash:ident) => {
        pub struct $name(pub [u8; $bytes]);

        impl $name {
            pub fn from<E: Encodable>(obj: &E) -> Option<Self> {
                let serialized = encode(obj, Infinite);
                match serialized {
                    Ok(ref serialized) => {
                        let mut buf: [u8; $bytes] = [0; $bytes];
                        buf.copy_from_slice(&$hash($bytes, &[], serialized).as_bytes());

                        Some($name(buf))
                    },
                    Err(_) => None
                }
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &$name) -> bool {
                (&self.0[..]).eq(&other.0[..])
            }
        }

        impl Eq for $name { }

        impl Copy for $name { }
        impl Clone for $name {
            fn clone(&self) -> $name {
                *self
            }
        }

        impl Encodable for $name {
            fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
                for i in 0..$bytes {
                    try!(s.emit_u8(self.0[i]));
                }

                Ok(())
            }
        }

        impl Decodable for $name {
            fn decode<S: Decoder>(s: &mut S) -> Result<$name, S::Error> {
                let mut buf = [0; $bytes];

                for i in 0..$bytes {
                    buf[i] = try!(s.read_u8());
                }

                Ok($name(buf))
            }
        }
    }
}

digest_impl!(Digest512, 64, blake2b);

impl Digest512 {
    pub fn interpret(&self) -> Fr {
        Fr::interpret(&self.0)
    }
}

#[test]
fn digest_string_repr() {
    use super::secrets::*;

    let rng = &mut ::rand::thread_rng();

    let privkey = PrivateKey::new(rng);
    
    for _ in 0..100 {
        let pubkey = privkey.pubkey(rng);
        let comm = pubkey.hash();
        let string = comm.to_string();
        let newcomm = Digest256::from_string(&string).unwrap();

        assert!(comm == newcomm);
    }

    assert!(Digest256::from_string("2b8c8iK5PGtStZzEz45ycJSQLq1RPXGkjqmWAM1Q8jQ4dqVHkY").is_some());

    assert!(Digest256::from_string("2b8c8iK5PGtStZzEz45ycJSQLq1RPXGkjqmWAM1Q8jQ4dqVHkS").is_none());
    assert!(Digest256::from_string("2b8c8iK5PGtStZzEz45ycJSQLq1RPXGkjqmWAM2Q8jQ4dqVHkY").is_none());
    assert!(Digest256::from_string("1b8c8iK5PGtStZzEz45ycJSQLq1RPXGkjqmWAM1Q8jQ4dqVHkY").is_none());
}
