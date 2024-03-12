use std::fmt::Display;

use crate::{EthBlock, JsonStruct};
use prost_wkt_types::Value;
use rhai::serde::to_dynamic;
use rhai::Array;
use rhai::Map;
use rhai::Shared;
use rhai::{Dynamic, Engine};
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use serde_json::json;
use std::rc::Rc;
use substreams::pb::substreams::module::Input;
use substreams::pb::substreams::module_progress::Type;
use substreams::prelude::*;
use substreams::Hex;
use substreams_ethereum::Event;

use ethabi::Event as EthEvent;

pub fn get_events<T>(block: &mut EthBlock) -> Vec<Dynamic>
where
    T: Sized + Event + Clone + Serialize,
{
    //let addresses = addresses.iter().map(|address| Hex(address)).collect::<Vec<_>>();
    let mut events = vec![];

    for log in block.logs() {
        let event = T::match_and_decode(log);

        if let Some(event) = event {
            let as_value = serde_json::to_value(event);
            match as_value {
                Ok(val) => {
                    if !val.is_null() {
                        events.push(serde_json::from_value(val).unwrap())
                    }
                }
                Err(err) => substreams::log::println(format!(
                    "GOT ERROR CONVERTING EVENT INTO DYNAMIC: {err:?}"
                )),
            }
        }
    }

    events
}

trait TypeRegister {
    fn register_types(engine: &mut Engine);
}

impl TypeRegister for Deltas<DeltaProto<JsonStruct>> {
    fn register_types(engine: &mut Engine) {
        engine.register_type::<Self>().register_get(
            "deltas",
            |obj: &mut Deltas<DeltaProto<JsonStruct>>| {
                let deltas = obj
                    .deltas
                    .iter()
                    .map(|delta| {
                        //let old_value = serde_json::to_string(&delta.old_value).unwrap();
                        //let old_value: serde_json::Map<_, _> =
                        //serde_json::from_str(&old_value).unwrap();
                        //let old_value: rhai::Map = serde_json::from_value(old_value).unwrap();

                        let new_value = serde_json::to_value(&delta.new_value).unwrap();
                        let new_value: rhai::Map = serde_json::from_value(new_value).unwrap();

                        let mut obj = Map::new();
                        obj.insert("operation".into(), (delta.operation as i64).into());
                        obj.insert("ordinal".into(), (delta.ordinal as i64).into());
                        obj.insert("key".into(), delta.key.clone().into());
                        obj.insert(
                            "oldValue".into(),
                            to_dynamic(delta.old_value.clone()).unwrap(),
                        );
                        obj.insert("newValue".into(), Dynamic::from_map(new_value));
                        Dynamic::from_map(obj)
                    })
                    .collect::<Vec<Dynamic>>();
                Dynamic::from_array(deltas)
            },
        );
    }
}

impl TypeRegister for DeltaProto<JsonStruct> {
    fn register_types(engine: &mut Engine) {
        engine.register_type::<Self>();
    }
}

impl TypeRegister for JsonStruct {
    fn register_types(engine: &mut Engine) {
        engine.register_type::<JsonStruct>().register_indexer_get(
            |obj: &mut JsonStruct, property: &str| -> Dynamic {
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
            },
        );
    }
}

impl TypeRegister for Vec<u8> {
    fn register_types(engine: &mut Engine) {
        // register the address type
        engine
            .register_type_with_name::<Vec<u8>>("Address")
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
        engine
            .register_type_with_name::<BigInt>("Uint")
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

impl TypeRegister for Rc<StoreSetProto<JsonStruct>> {
    fn register_types(engine: &mut Engine) {
        type StoreSet = Rc<StoreSetProto<JsonStruct>>;
        let set_fn = |store: &mut StoreSet, key: Dynamic, value: Dynamic| {
            let error_msg = format!("Couldn't cast!Key: {:?}, Value: {:?}", &key, value);

            // TODO Add support for storing scalar values
            if let (Some(key), Some(value)) =
                (key.try_cast::<String>(), value.try_cast::<JsonStruct>())
            {
                store.set(0, key, &value);
            } else {
                substreams::log::println(error_msg);
            }
        };

        let set_many_fn = |store: &mut StoreSet, keys: Array, value: Dynamic| {
            let keys: Vec<String> = keys
                .into_iter()
                .map(|e| {
                    e.try_cast::<String>()
                        .expect("COULDN'T CONVERT THE KEY INTO A STRING!")
                })
                .collect::<Vec<_>>();

            // TODO Add support for storing scalar values
            if let Some(value) = value.try_cast::<JsonStruct>() {
                store.set_many(0, &keys, &value);
            }
        };

        let delete_fn = |store: &mut StoreSet, prefix: Dynamic| {
            if let Some(prefix) = prefix.try_cast::<String>() {
                store.delete_prefix(0, &prefix)
            }
        };

        engine
            .register_type_with_name::<Rc<StoreSetProto<JsonStruct>>>("StoreSet")
            .register_fn("set", set_fn)
            .register_fn("setMany", set_many_fn)
            .register_fn("delete_prefix", delete_fn);
    }
}

impl TypeRegister for Rc<StoreSetIfNotExistsProto<JsonStruct>> {
    fn register_types(engine: &mut Engine) {
        type StoreSetOnce = Rc<StoreSetIfNotExistsProto<JsonStruct>>;

        let set_fn = |store: &mut StoreSetOnce, key: Dynamic, value: Dynamic| {
            let error_msg = format!("key: {:?}, value: {:?}", &key, &value);
            if let (Some(key), Some(value)) =
                (key.try_cast::<String>(), value.try_cast::<JsonStruct>())
            {
                substreams::log::println(format!("Key: {:?}, Value: {:?}", &key, value));
                store.set_if_not_exists(0, key, &value);
            } else {
                panic!("{}", error_msg)
            }
        };

        let set_many_fn = |store: &mut StoreSetOnce, keys: Array, value: Dynamic| {
            let keys: Vec<String> = keys
                .into_iter()
                .map(|e| {
                    e.try_cast::<String>()
                        .expect("COULDN'T CONVERT THE KEY INTO A STRING!")
                })
                .collect::<Vec<_>>();

            if let Some(value) = value.try_cast::<JsonStruct>() {
                store.set_if_not_exists_many(0, &keys, &value);
            }
        };

        let delete_fn = |store: &mut StoreSetOnce, prefix: Dynamic| {
            if let Some(prefix) = prefix.try_cast::<String>() {
                store.delete_prefix(0, &prefix)
            }
        };

        engine
            .register_type_with_name::<StoreSetOnce>("StoreSetOnce")
            .register_fn("set", set_fn)
            .register_fn("setOnce", set_fn)
            .register_fn("setMany", set_many_fn)
            .register_fn("setOnceMany", set_many_fn)
            .register_fn("deletePrefix", delete_fn);
    }
}

impl TypeRegister for Rc<StoreGetProto<JsonStruct>> {
    fn register_types(engine: &mut Engine) {
        engine
            .register_type_with_name::<Self>("StoreGet")
            .register_fn("get", |store: &mut Self, key: String| {
                if let Some(value) = store.get_last(&key) {
                    value
                } else {
                    Default::default()
                }
            })
            .register_fn("get_first", |store: &mut Self, key: String| {
                if let Some(value) = store.get_first(&key) {
                    value
                } else {
                    Default::default()
                }
            });
    }
}

pub fn register_builtins(engine: &mut Engine) {
    <Vec<u8>>::register_types(engine);
    <BigInt>::register_types(engine);
    <JsonStruct>::register_types(engine);
    <Deltas<DeltaProto<JsonStruct>>>::register_types(engine);
    <Rc<StoreSetProto<JsonStruct>>>::register_types(engine);
    <Rc<StoreSetIfNotExistsProto<JsonStruct>>>::register_types(engine);
    <Rc<StoreGetProto<JsonStruct>>>::register_types(engine);
}
