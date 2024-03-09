use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serializer};
use substreams::scalar::BigInt;

pub mod big_int {
    use super::*;

    pub fn serialize<S: Serializer>(big_int: &BigInt, serializer: S) -> Result<S::Ok, S::Error> {
        let as_str = big_int.to_string();

        serializer.collect_str(&as_str)
    }


    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<BigInt, D::Error> {
        let as_string = String::deserialize(de)?;
        BigInt::from_str(&as_string).map_err(serde::de::Error::custom)
    }

    pub mod vec {
        use super::*;

        pub fn serialize<S: Serializer>(vec: &Vec<BigInt>, serializer: S) -> Result<S::Ok, S::Error> {
            let vec = vec.into_iter().map(|n| n.to_string()).collect::<Vec<_>>();

            serializer.collect_seq(vec)
        }


        pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<BigInt>, D::Error> {
            let seq = <Vec<String>>::deserialize(de)?;
            let mut output = vec![];

            for item in seq.iter() {
                match BigInt::from_str(item) {
                    Ok(val) => output.push(val),
                    Err(err) => return Err(serde::de::Error::custom(err.to_string())),
                };
            }

            Ok(output)
        }
    }
}

pub mod bytes {
    use substreams::Hex;

    use super::*;

    pub fn serialize<S: Serializer>(val: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        let as_str = format!("0x{}", Hex(val).to_string());

        serializer.collect_str(&as_str)
    }


    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<u8>, D::Error> {
        let as_string = String::deserialize(de)?;
        Hex::decode(&as_string).map_err(serde::de::Error::custom)
    }

    pub mod vec {
        use super::*;

        pub fn serialize<S: Serializer>(vec: &Vec<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error> {
            let vec = vec.into_iter().map(|val| Hex(val).to_string()).collect::<Vec<_>>();

            serializer.collect_seq(vec)
        }


        pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<Vec<u8>>, D::Error> {
            let seq = <Vec<String>>::deserialize(de)?;
            let mut output = vec![];

            for item in seq.iter() {
                match Hex::decode(item) {
                    Ok(val) => output.push(val),
                    Err(err) => return Err(serde::de::Error::custom(err.to_string())),
                };
            }

            Ok(output)
        }
    }
}
