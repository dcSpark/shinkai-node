use chrono::{Utc, DateTime};
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct ShinkaiTime {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl ShinkaiTime {
    #[wasm_bindgen(js_name = generateTimeNow)]
    pub fn generate_time_now() -> String {
        shinkai_message_primitives::schemas::shinkai_time::ShinkaiTime::generate_time_now()
    }

    #[wasm_bindgen(js_name = generateTimeInFutureWithSecs)]
    pub fn generate_time_in_future_with_secs(secs: i64) -> String {
        shinkai_message_primitives::schemas::shinkai_time::ShinkaiTime::generate_time_in_future_with_secs(secs)
    }

    #[wasm_bindgen(js_name = generateSpecificTime)]
    pub fn generate_specific_time(year: i32, month: u32, day: u32, hr: u32, min: u32, sec: u32) -> String {
        shinkai_message_primitives::schemas::shinkai_time::ShinkaiTime::generate_specific_time(year, month, day, hr, min, sec)
    }
}
