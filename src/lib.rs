#![allow(dead_code, unused)]

use std::collections::HashMap;

use prost_wkt_types::{Any, ListValue, Struct};
use serde_json::{value::Index, Map, Value};
use substreams::prelude::*;
use substreams::Hex;
use substreams_database_change::pb::database::DatabaseChanges;
use substreams_entity_change::pb::entity::EntityChanges;
use substreams_ethereum::block_view::LogView;
use substreams_ethereum::pb::eth::v2::{self as eth, Block};
use substreams_ethereum::Event;

use std::rc::Rc;

pub mod abis;
pub mod custom_serde;

use rhai::{
    export_module, exported_module,
    serde::{from_dynamic, to_dynamic},
    Dynamic, Engine, Scope,
};

use prost::Message;
use serde::{Deserialize, Serialize, Serializer};

pub type EthBlock = Block;
pub type JsonValue = prost_wkt_types::Value;

include!("./generated/engine_init.rs");

include!("./generated/imports.rs");

const RHAI_SCRIPT: &str = include_str!("../streamline.rhai");

include!("./streamline.rs");
