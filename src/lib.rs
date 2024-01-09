use std::collections::HashMap;

use alloy_primitives::aliases::{B16, B32, B8, U1};
use alloy_primitives::{address, hex_literal, Address, Bytes, FixedBytes, Log};
use alloy_sol_types::abi::{Decoder, TokenSeq};
use alloy_sol_types::sol_data::Bool;
use alloy_sol_types::{SolCall, SolEnum, SolEvent, SolInterface, TopicList};
use prost_wkt_types::{ListValue, Struct};
use serde_json::{value::Index, Map, Value};
use substreams::prelude::*;
use substreams::Hex;
use substreams_database_change::pb::database::DatabaseChanges;
use substreams_ethereum::block_view::LogView;
use substreams_ethereum::pb::eth::v2::{self as eth, Block};
use substreams_ethereum::Event;

use serde::{Deserialize, Serialize, Serializer};

use substreams_alloy_helpers::{
    filter, format_inputs, loose_sol, map, map_access, map_insert, map_literal, prelude::*,
    sol_type, to_array, to_map, with_map,
};
use substreams_ethereum::pb::eth::rpc::{RpcCall, RpcCalls};

include!("./streamline.rs");
