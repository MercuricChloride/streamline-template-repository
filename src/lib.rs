use std::collections::HashMap;

use prost_wkt_types::{ListValue, Struct};
use serde_json::{value::Index, Map, Value};
use substreams::prelude::*;
use substreams::Hex;
use substreams_database_change::pb::database::DatabaseChanges;
use substreams_ethereum::block_view::LogView;
use substreams_ethereum::pb::eth::v2::{self as eth, Block};
use substreams_ethereum::Event;

use serde::{Deserialize, Serialize, Serializer};

const RHAI_SCRIPT: &str = include_str!("../streamline.rhai");

include!("./streamline.rs");
