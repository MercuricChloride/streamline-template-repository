use std::fmt::Display;

use prost_wkt_types::Value;
use rhai::Map;
use rhai::Shared;
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use substreams::pb::substreams::module::Input;
use substreams::pb::substreams::module_progress::Type;
use substreams_ethereum::Event;
use crate::{EthBlock, JsonStruct};
use rhai::{Engine, Dynamic};
use substreams::prelude::*;
use substreams::Hex;

use ethabi::Event as EthEvent;

pub fn get_events<T>(block: &mut EthBlock) -> Vec<Dynamic>
where T: Sized + Event + Clone {
    //let addresses = addresses.iter().map(|address| Hex(address)).collect::<Vec<_>>();
    let mut events = vec![];

    for log in block.logs() {
        let event = T::match_and_decode(log);

        if let Some(event) = event {
            //let as_dyn = Dynamic::from(event.clone());
            //if !as_dyn.is_unit() {
                //events.push(as_dyn);
            //}
        }
    }

    events
}

trait TypeRegister {
    fn register_types(engine: &mut Engine);
}

impl TypeRegister for Deltas<DeltaProto<JsonStruct>> {
    fn register_types(engine: &mut Engine) {
        engine.register_type::<Self>()
        .register_get("deltas", |obj: &mut Deltas<DeltaProto<JsonStruct>>| {
            let deltas = obj.deltas.clone();
            Dynamic::from(deltas)
        });
    }
}

impl TypeRegister for DeltaProto<JsonStruct> {
    fn register_types(engine: &mut Engine) {
        engine.register_type::<Self>();
    }
}

impl TypeRegister for JsonStruct {
    fn register_types(engine: &mut Engine) {
        engine.register_type::<JsonStruct>()
            .register_indexer_get(|obj: &mut JsonStruct, property: &str| -> Dynamic {
                let field = obj.fields.get("result").unwrap();
                let obj = serde_json::to_value(field).unwrap();
                let obj: rhai::Map = serde_json::from_value(obj).unwrap();
                if let Some(value) = obj.get(property) {
                    value.clone()
                } else {
                    Dynamic::from(())
                }
                // // TODO Do some massaging of nested indexes
                // let results = obj.fields.get("result").unwrap();

                // match results.kind.clone().unwrap() {
                //     prost_wkt_types::value::Kind::StructValue(val) => {
                //         let val = val.fields.get(property).unwrap();
                //         Dynamic::from(val.clone())
                //     },
                //     prost_wkt_types::value::Kind::ListValue(val) => {
                //         let index = property.parse::<usize>().unwrap();
                //         let value = val.values.get(index).unwrap();
                //         Dynamic::from(value.clone())
                //     }
                //     _ => Dynamic::from("invalid property access for JsonStruct")
                // }
            });
    }
}

impl TypeRegister for Vec<u8> {
    fn register_types(engine: &mut Engine) {
        // register the address type
        engine.register_type_with_name::<Vec<u8>>("Address")
            .register_fn("address", |x: Vec<u8>| {
                if x.len() == 20 {
                    Dynamic::from(format!("0x{}", Hex(x).to_string()))
                } else {
                    Dynamic::from(())
                }
            });
    }
}

impl TypeRegister for BigInt {
        fn register_types(engine: &mut Engine) {
            engine.register_type_with_name::<BigInt>("Uint")
                .register_fn("uint", |x: BigInt| x.to_string())
                .register_fn("uint", |x: Dynamic| {
                    let as_string = x.to_string();
                    if let Ok(value) = BigInt::try_from(as_string) {
                        Dynamic::from(value)
                    } else {
                        Dynamic::from(())
                    }
                });
        }
}

struct MyBigInt {
    pub big_int: BigInt,
}

impl From<MyBigInt> for BigInt {
    fn from(value: MyBigInt) -> Self {
        value.big_int
    }
}

#[derive(Serialize, Deserialize)]
struct SerdeTest {
    #[serde(with = "crate::builtins::serde_big_int")]
    pub big_int: BigInt,
}

pub mod serde_big_int {
    use std::str::FromStr;

    use serde::Deserializer;

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


pub fn register_builtins(engine: &mut Engine) {
    <Vec<u8>>::register_types(engine);
    BigInt::register_types(engine);
    <JsonStruct>::register_types(engine);
    <DeltaProto<JsonStruct>>::register_types(engine);
    <Deltas<DeltaProto<JsonStruct>>>::register_types(engine);
}
