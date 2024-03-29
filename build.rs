use std::{fmt::format, fs};

use serde_json::{self, Value};
use serde::{de, Deserialize, Serialize};
use substreams_ethereum::Abigen;

use convert_case::{Case, Casing};

trait AbiEventHelpers {
    fn event_getter(&self, abi_module_name: &str) -> String;
    fn type_builder(&self, abi_module_name: &str) -> String;
}

impl AbiEventHelpers for Value {
    fn event_getter(&self, abi_module_name: &str) -> String {
        let event_name = self["name"].as_str().unwrap().to_case(Case::UpperCamel);
        format!(r#"
        pub fn {event_name}(block: &mut EthBlock, addresses: Array) -> Dynamic {{
            let events = get_events::<{abi_module_name}::events::{event_name}>(block);
            if events.is_empty() {{
                Dynamic::UNIT
            }} else {{
                let events = events.into_iter().map(Dynamic::from).collect::<Vec<_>>();
                Dynamic::from(events)
            }}
        }}
        "#)
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
    fn engine_init(&self, abi_path: &str, contract_name: &str, events: &Vec<&Value>) -> String;
    fn build_type(&self, abi_module_name: &str) -> String;
}

impl AbiHelpers for Vec<Value> {
    fn engine_init(&self, abi_path: &str, contract_name: &str, events: &Vec<&Value>) -> String {
        let abi_module_name = format!("{}_abi", contract_name);

        let event_registers = events.iter()
            .map(|event| event.event_getter(&abi_module_name))
            .collect::<Vec<_>>()
            .join("\n");

        format!(r#"
#[export_module]
mod {contract_name} {{
    use super::EthBlock;
    use rhai::{{Array, Dynamic}};
    use rhai::plugin::*;
    use substreams_ethereum::Event;
    use crate::abis::{contract_name} as {abi_module_name};
    use crate::builtins::get_events;
    {event_registers}
}}
        "#)
    }

    fn build_type(&self, abi_module_name: &str) -> String {
        let type_builders = self.iter()
            .map(|event| event.type_builder(abi_module_name))
            .collect::<Vec<_>>()
            .join("\n");

        type_builders
    }
}
// Derives
const DEFAULT_DERIVES: &'static str = "#[derive(Debug, Clone, PartialEq)]";
const REPLACEMENT_DERIVES: &'static str = "#[derive(Debug, Clone, PartialEq, CustomType, Serialize, Deserialize)]";

// Imports
const DEFAULT_IMPORTS: &'static str = "use super::INTERNAL_ERR;";
const REPLACEMENT_IMPORTS: &'static str = "use super::INTERNAL_ERR; use rhai::{{TypeBuilder, CustomType}}; use serde::{Serialize, Deserialize};";

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
        if is_field_def(line)
            && line.contains("substreams::scalar::BigInt,") {
                new_lines.push("#[serde(with = \"crate::custom_serde::big_int\")]");
        }

        if is_field_def(line)
            && line.contains("Vec<substreams::scalar::BigInt>,"){
                new_lines.push("#[serde(with = \"crate::custom_serde::big_int::vec\")]");
        }

        if is_field_def(line)
            && line.contains("Vec<u8>,"){
                new_lines.push("#[serde(with = \"crate::custom_serde::bytes\")]");
        }

        if is_field_def(line)
            && line.contains("Vec<Vec<u8>>,"){
                new_lines.push("#[serde(with = \"crate::custom_serde::bytes::vec\")]");
        }

        new_lines.push(line);
    }

    fs::write(path, new_lines.join("\n")).unwrap();
}

pub fn main() -> Result<(), anyhow::Error> {
    let abis = fs::read_dir("abis").unwrap();

    let mut module_formatters = String::new();
    let mut imports = String::new();
    let mut mod_file = String::new();

    for abi_path in abis {
        let abi_path = abi_path?.path();
        let abi_path_str = abi_path.to_str().unwrap();
        let abi_file_name = abi_path.file_name().unwrap().to_str().unwrap();

        let abi_contents = fs::read_to_string(&abi_path_str)?;

        let abi_name = abi_file_name.split('/').last().unwrap().trim_end_matches(".json");
        let decoded: Vec<Value>  = serde_json::from_str(&abi_contents)?;

        // Write the rust bindings
        let target_path = format!("./src/abis/{}.rs",abi_name);
        Abigen::new(abi_name, abi_path_str)?
            .generate()?
            .write_to_file(&target_path)?;

        // Replace the default derives with the custom ones
        replace_derives(&target_path);
        replace_imports(&target_path);
        add_bigint_serde(&target_path);

        // Add the abi module to the mod file
        mod_file.push_str(&format!(r#"pub mod {abi_name};"#));

        let events = decoded.iter().filter(|event| event["type"] == "event").collect::<Vec<_>>();

        let generated_code = decoded.engine_init(&abi_path_str, abi_name, &events);

        // Write the generated rhai module to a file
        fs::write(format!("./src/generated/{}.rs", abi_name), generated_code).unwrap();

        // import the include statement to the imports string
        imports.push_str(&format!(r#"include!("./{abi_name}.rs");"#));

        module_formatters.push_str(&format!(r#"
let module = exported_module!({abi_name});
engine.register_static_module("{abi_name}", module.into());"#));

        // build the types with the engine
        module_formatters.push_str(&decoded.build_type(&abi_name));
    }

    let engine_init_macro = format!(r#"
macro_rules! engine_init {{
    () => {{{{
        let mut engine = Engine::new_raw();
        let mut scope = Scope::new();
        let (mut engine, mut scope) = ::rhai::packages::streamline::init_package(engine, scope);
        {module_formatters}
        (engine, scope)
    }}}};
}}
    "#);

     fs::write("./src/generated/imports.rs", imports).unwrap();
     fs::write("./src/generated/engine_init.rs", engine_init_macro).unwrap();
     fs::write("./src/abis/mod.rs", mod_file).unwrap();

    Ok(())
}
