use chrono::Utc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct ShinkaiTime {}

#[wasm_bindgen]
impl ShinkaiTime {
    #[wasm_bindgen(js_name = generateTimeNow)]
    pub fn generate_time_now() -> String {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S.%f").to_string();
        let scheduled_time = format!("{}Z", &timestamp[..23]);
        scheduled_time
    }
}