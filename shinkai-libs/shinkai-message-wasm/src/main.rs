pub mod shinkai_utils;
pub mod shinkai_wasm_wrappers;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");
    Ok(())
}
