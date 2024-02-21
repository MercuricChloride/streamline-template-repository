use std::{fmt::format, fs};

use serde_json::{self, Value};
use serde::{de, Deserialize, Serialize};
use substreams_ethereum::Abigen;

trait AbiEventHelpers {
    fn event_getter(&self, abi_module_name: &str) -> String;
    fn type_builder(&self, abi_module_name: &str) -> String;
}

impl AbiEventHelpers for Value {
    fn event_getter(&self, abi_module_name: &str) -> String {
        let event_name = self["name"].as_str().unwrap();
        format!(r#"
        pub fn {event_name}(block: &mut EthBlock, addresses: Array) -> Dynamic {{
            let events = get_events::<{abi_module_name}::events::{event_name}>(block);
            let events = events.into_iter().map(Dynamic::from).collect::<Vec<_>>();
            Dynamic::from(events)
        }}
        "#)
    }
    
    fn type_builder(&self, abi_module_name: &str) -> String {
        if self["type"] != "event" {
            return String::new();
        }
        let event_name = self["name"].as_str().unwrap();
        format!("engine.build_type::<abi::{abi_module_name}::events::{event_name}>();")
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
    use crate::abi::{contract_name} as {abi_module_name};
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
const REPLACEMENT_DERIVES: &'static str = "#[derive(Debug, Clone, PartialEq, CustomType)]";

// Imports
const DEFAULT_IMPORTS: &'static str = "use super::INTERNAL_ERR;";
const REPLACEMENT_IMPORTS: &'static str = "use super::INTERNAL_ERR; use rhai::{{TypeBuilder, CustomType}};";

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

pub fn main() -> Result<(), anyhow::Error> {
    let abis = fs::read_dir("abi").unwrap();

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
        let target_path = format!("./src/abi/{}.rs",abi_name);
        Abigen::new(abi_name, abi_path_str)?
            .generate()?
            .write_to_file(&target_path)?;

        // Replace the default derives with the custom ones
        replace_derives(&target_path);
        replace_imports(&target_path);

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
    // A module can simply be registered into the global namespace.
    engine.register_static_module("{abi_name}", module.into());
            "#));

        // build the types with the engine
        module_formatters.push_str(&decoded.build_type(&abi_name));
    }

    let engine_init_macro = format!(r#"
macro_rules! engine_init {{
    () => {{{{
        let mut engine = Engine::new();
        let mut scope = Scope::new();
        {module_formatters}
        (engine, scope)
    }}}};
}}
    "#);

     fs::write("./src/generated/imports.rs", imports).unwrap();
     fs::write("./src/generated/engine_init.rs", engine_init_macro).unwrap();
     fs::write("./src/abi/mod.rs", mod_file).unwrap();

    Ok(())
}