use std::fs;

use serde_json::{self, Value};
use serde::{de, Deserialize, Serialize};

trait AbiHelpers {
    fn engine_init(&self, abi_path: &str, contract_name: &str, events: &Vec<&Value>) -> String;
}

impl AbiHelpers for Vec<Value> {
    fn engine_init(&self, abi_path: &str, contract_name: &str, events: &Vec<&Value>) -> String {
        let abi_module_name = format!("{}_abi", contract_name);

        let event_registers = events.iter().map(|event| {
            let event_name = event["name"].as_str().unwrap();
            format!(r#"
            pub fn {event_name}(block: &mut EthBlock, addresses: Array) -> Dynamic {{
                Dynamic::from(get_events::<{abi_module_name}::events::{event_name}>(block.clone()))
            }}
            "#)
        })
        .collect::<Vec<_>>()
        .join("\n");

        let register_all_events = events.iter().map(|event| {
            let event_name = event["name"].as_str().unwrap();
            format!("register_{event_name}(engine);")
        })
        .collect::<Vec<_>>()
        .join("\n");


        format!(r#"
::substreams_ethereum::use_contract!({abi_module_name}, "{abi_path}");

#[export_module]
mod {contract_name} {{
    use super::SharedBlock as EthBlock;
    use rhai::{{Array, Dynamic}};
    use rhai::plugin::*;
    use substreams_ethereum::Event;
    use super::{abi_module_name};
    use crate::get_events;
    {event_registers}
}}
        "#)
    }
}


pub fn main() -> Result<(), anyhow::Error> {
    let abis = fs::read_dir("abi").unwrap();

    let mut module_formatters = String::new();

    let mut imports = String::new();

    for abi_path in abis {
        let abi_path = abi_path?.path();
        let abi_path_str = abi_path.to_str().unwrap();
        let abi_file_name = abi_path.file_name().unwrap().to_str().unwrap();

        let abi_contents = fs::read_to_string(&abi_path)?;

        let abi_name = abi_file_name.split('/').last().unwrap().trim_end_matches(".json");
        let decoded: Vec<Value>  = serde_json::from_str(&abi_contents)?;

        let events = decoded.iter().filter(|event| event["type"] == "event").collect::<Vec<_>>();

        // we need to go up two directories because the path is different from the root compared to the 
        // generated file
        //let abi_path_str = format!("../../{}", abi_path_str);
        let generated_code = decoded.engine_init(&abi_path_str, abi_name, &events);

        fs::write(format!("./src/generated/{}.rs", abi_name), generated_code).unwrap();

        // import the include statement to the imports string
        imports.push_str(&format!(r#"include!("./{abi_name}.rs");"#));

        module_formatters.push_str(&format!(r#"
    let module = exported_module!({abi_name});
    // A module can simply be registered into the global namespace.
    engine.register_static_module("{abi_name}",module.into());
            "#));
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

    Ok(())
}