use std::{env, fmt::format, fs};

use serde::{de, Deserialize, Serialize};
use serde_json::{self, Value};
use substreams_ethereum::Abigen;

use convert_case::{Case, Casing};

trait AbiEventHelpers {
    fn event_getter(&self, abi_module_name: &str) -> String;
    fn type_builder(&self, abi_module_name: &str) -> String;
}

impl AbiEventHelpers for Value {
    fn event_getter(&self, abi_module_name: &str) -> String {
        let event_name = self["name"].as_str().unwrap().to_case(Case::UpperCamel);
        format!(
            r#"
        pub fn {event_name}(block: &mut EthBlock, addresses: Array) -> Array {{
            let events = get_events::<{abi_module_name}::events::{event_name}>(block, addresses);
            if events.is_empty() {{
                vec![].into()
            }} else {{
                let events = events.into_iter().map(Dynamic::from).collect::<Vec<_>>();
                events.into()
            }}
        }}
        "#
        )
    }

    fn type_builder(&self, abi_module_name: &str) -> String {
        if self["type"] != "event" {
            return String::new();
        }
        let event_name = self["name"].as_str().unwrap().to_case(Case::UpperCamel);
        format!("engine.build_type::<abis::{abi_module_name}::events::{event_name}>();")
    }
}

trait AbiHelpers {
    fn engine_init(
        &self,
        abi_path: &str,
        contract_name: &str,
        events: &Vec<&Value>,
        functions: &str,
    ) -> String;
    fn build_type(&self, abi_module_name: &str) -> String;
}

impl AbiHelpers for Vec<Value> {
    fn engine_init(
        &self,
        abi_path: &str,
        contract_name: &str,
        events: &Vec<&Value>,
        functions: &str,
    ) -> String {
        let abi_module_name = format!("{}_abi", contract_name);

        let event_registers = events
            .iter()
            .map(|event| event.event_getter(&abi_module_name))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"
#[export_module]
mod {contract_name} {{
    use super::EthBlock;
    use rhai::{{Array, Dynamic}};
    use rhai::plugin::*;
    use substreams_ethereum::Event;
    use crate::abis::{contract_name} as {abi_module_name};
    use crate::custom_serde;
    use rhai::packages::streamline::builtins::*;
    {event_registers}
    {functions}
}}
        "#
        )
    }

    fn build_type(&self, abi_module_name: &str) -> String {
        let type_builders = self
            .iter()
            .map(|event| event.type_builder(abi_module_name))
            .collect::<Vec<_>>()
            .join("\n");

        type_builders
    }
}

struct AbiViewFunction(Value);

struct FunctionInput {
    pub name: String,
    pub kind: String,
}

impl AbiViewFunction {
    pub fn new(value: &Value) -> Option<Self> {
        if let (Some(Value::String(mutability)), Some(Value::String(kind))) =
            (value.get("stateMutability"), value.get("type"))
        {
            if kind.as_str() != "function" {
                return None;
            }

            match mutability.as_str() {
                "view" | "pure" => return Some(Self(value.clone())),
                _ => return None,
            };
        };
        None
    }

    pub fn generate(&self, abi_name: &str) -> String {
        let abi_module_name = format!("{}_abi", abi_name);
        let function_name = self.0.get("name");

        let function_name = if let Some(Value::String(string)) = function_name {
            string
        } else {
            panic!("Function name not a string! {:?}", function_name)
        };

        let function_input_struct = function_name.to_case(Case::UpperCamel);

        let inputs = if let Some(Value::Array(arr)) = self.0.get("inputs") {
            arr.iter()
                .map(|e| {
                    let name = e.get("name");
                    let kind = e.get("type");

                    if let (Some(Value::String(name)), Some(Value::String(kind))) = (name, kind) {
                        FunctionInput {
                            name: name.into(),
                            kind: kind.into(),
                        }
                    } else {
                        panic!(
                            "Name or type not found to be a string! name: {:?}\n kind:{:?}",
                            name, kind
                        );
                    }
                })
                .collect()
        } else {
            vec![]
        };

        let outputs = if let Some(Value::Array(arr)) = self.0.get("outputs") {
            arr.iter()
                .map(|e| {
                    let kind = e.get("type");
                    let components = e.get("components");

                    if let Some(Value::String(kind)) = kind {
                        get_rust_type(kind, components)
                    } else {
                        panic!("ABI Output type not found to be a string! type:{:?}", kind);
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        } else {
            String::new()
        };

        if inputs.len() > 0 {
            // TODO This is for simplicity sake
            // I will support any function later, but for now the types are causing
            // a lot of problems, and I don't have time to solve it perfectly right now
            return "".into();
        }

        format!(
            r#"
        pub fn {function_name}(target_address: ImmutableString) -> Dynamic {{
            type T = {abi_module_name}::functions::{function_input_struct};

            let call_result = rpc_call::<T, _>(T {{}}, target_address);
            if let Some(call_result) = call_result {{
                Dynamic::from(call_result)
            }} else {{
                Dynamic::UNIT
            }}
        }}
        "#
        )
    }
}

/// Converts a solidity type, into it's appropriate rust type
fn get_rust_type(kind: &str, tuple_components: Option<&Value>) -> String {
    match kind {
        "address" => "Vec<u8>".into(),
        //"uint8" | "uint16" | "uint24" | "uint32" => "u32".into(),
        //"int8" | "int16" | "int24" | "int32" => "i32".into(),
        "string" => "String".into(),
        "bool" => "bool".into(),
        s if s.contains("[]") => {
            let kind = s.split("[]").collect::<Vec<_>>()[0];
            format!("Vec<{}>", get_rust_type(kind, tuple_components))
        }
        s if s.contains("tuple") => {
            if let Some(Value::Array(parts)) = tuple_components {
                let parts = parts
                    .iter()
                    .filter_map(|e| {
                        if let Some(Value::String(e)) = e.get("type") {
                            Some(get_rust_type(e, tuple_components))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("({parts})")
            } else {
                panic!()
            }
        }
        s if s.contains("bytes") => "Vec<u8>".into(),
        s if s.contains("uint") => "BigInt".into(),
        s if s.contains("int") => "BigInt".into(),
        _ => panic!("Unknown type! {:?}", kind),
    }
}

// Derives
const DEFAULT_DERIVES: &'static str = "#[derive(Debug, Clone, PartialEq)]";
const REPLACEMENT_DERIVES: &'static str =
    "#[derive(Debug, Clone, PartialEq, CustomType, Serialize, Deserialize, From)]";

// Imports
const DEFAULT_IMPORTS: &'static str = "use super::INTERNAL_ERR;";
const REPLACEMENT_IMPORTS: &'static str = "use super::INTERNAL_ERR; use rhai::{{TypeBuilder, CustomType}}; use serde::{Serialize, Deserialize}; use derive_more::From;";

fn replace_derives(path: &str) {
    let mut file = fs::read_to_string(path).unwrap();
    let contents = file.replace(DEFAULT_DERIVES, REPLACEMENT_DERIVES);
    fs::write(path, contents).unwrap();
}

fn replace_imports(path: &str) {
    let mut file = fs::read_to_string(path).unwrap();
    let contents = file.replace(DEFAULT_IMPORTS, REPLACEMENT_IMPORTS);
    fs::write(path, contents).unwrap();
}

// NOTE This is pretty dumb, I should use regex eventually here but it's fine for me now
fn is_field_def(line: &str) -> bool {
    let line = line.trim();

    // make sures it's a public field
    line.starts_with("pub")
        // make sure it isn't a function
        && !line.starts_with("pub fn")
        && !line.contains("->")
}

fn add_bigint_serde(path: &str) {
    let file = fs::read_to_string(path).unwrap();
    let mut new_lines = vec![];
    let lines = file.lines().collect::<Vec<_>>();

    for line in lines {
        // we want to replace field declarations of BigInts, with the appropriate derives
        // if line.trim() == "substreams::scalar::BigInt," {
        //     new_lines.push("#[serde_as(as = \"DisplayFromStr\")]");
        // }

        if is_field_def(line) && line.contains("substreams::scalar::BigInt,") {
            new_lines.push("#[serde(with = \"crate::custom_serde::big_int\")]");
        }

        if is_field_def(line) && line.contains("Vec<substreams::scalar::BigInt>,") {
            new_lines.push("#[serde(with = \"crate::custom_serde::big_int::vec\")]");
        }

        if is_field_def(line) && line.contains("Vec<u8>,") {
            new_lines.push("#[serde(with = \"crate::custom_serde::bytes\")]");
        }

        if is_field_def(line) && line.contains("Vec<Vec<u8>>,") {
            new_lines.push("#[serde(with = \"crate::custom_serde::bytes::vec\")]");
        }

        new_lines.push(line);
    }

    fs::write(path, new_lines.join("\n")).unwrap();
}

pub fn main() -> Result<(), anyhow::Error> {
    let home = env::var("HOME").expect("Couldn't get $HOME variable on path!");
    let abis_path = format!("{home}/streamline-cli/abis");
    let abis = fs::read_dir(abis_path).unwrap();

    let mut module_formatters = String::new();
    let mut imports = String::new();
    let mut mod_file = String::new();

    for abi_path in abis {
        let abi_path = abi_path?.path();
        let abi_path_str = abi_path.to_str().unwrap();
        let abi_file_name = abi_path.file_name().unwrap().to_str().unwrap();

        let abi_contents = fs::read_to_string(&abi_path_str)?;

        let abi_name = abi_file_name
            .split('/')
            .last()
            .unwrap()
            .trim_end_matches(".json");

        let decoded: Vec<Value> = serde_json::from_str(&abi_contents)?;

        // Write the rust bindings
        let target_path = format!("./src/abis/{}.rs", abi_name);
        Abigen::new(abi_name, abi_path_str)?
            .generate()?
            .write_to_file(&target_path)?;

        // Replace the default derives with the custom ones
        replace_derives(&target_path);
        replace_imports(&target_path);
        add_bigint_serde(&target_path);

        // Add the abi module to the mod file
        mod_file.push_str(&format!(r#"pub mod {abi_name};"#));

        let view_functions = decoded
            .iter()
            .filter_map(AbiViewFunction::new)
            .map(|e| e.generate(abi_name))
            .collect::<Vec<_>>()
            .join("");

        let events = decoded
            .iter()
            .filter(|event| event["type"] == "event")
            .collect::<Vec<_>>();

        let generated_code = decoded.engine_init(&abi_path_str, abi_name, &events, &view_functions);

        fs::create_dir_all("./src/generated/")?;
        // Write the generated rhai module to a file
        fs::write(format!("./src/generated/{}.rs", abi_name), generated_code).unwrap();

        // import the include statement to the imports string
        imports.push_str(&format!(r#"include!("./{abi_name}.rs");"#));

        module_formatters.push_str(&format!(
            r#"
let module = exported_module!({abi_name});
engine.register_static_module("{abi_name}", module.into());"#
        ));

        // build the types with the engine
        module_formatters.push_str(&decoded.build_type(&abi_name));
    }

    let engine_init_macro = format!(
        r#"
macro_rules! engine_init {{
    () => {{{{
        let mut engine = Engine::new_raw();
        let mut scope = Scope::new();
        let (mut engine, mut scope) = ::rhai::packages::streamline::init_package(engine, scope);
        {module_formatters}
        (engine, scope)
    }}}};
}}
    "#
    );

    fs::write("./src/generated/imports.rs", imports).unwrap();
    fs::write("./src/generated/engine_init.rs", engine_init_macro).unwrap();
    fs::write("./src/abis/mod.rs", mod_file).unwrap();

    Ok(())
}
