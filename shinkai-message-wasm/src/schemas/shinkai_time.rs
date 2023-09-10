use chrono::{Utc, DateTime};
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

    #[wasm_bindgen(js_name = generateTimeInFutureWithSecs)]
    pub fn generate_time_in_future_with_secs(secs: i64) -> String {
        let timestamp = (Utc::now() + chrono::Duration::seconds(secs))
            .format("%Y-%m-%dT%H:%M:%S.%f")
            .to_string();
        let scheduled_time = format!("{}Z", &timestamp[..23]);
        scheduled_time
    }

    #[wasm_bindgen(js_name = generateSpecificTime)]
    pub fn generate_specific_time(year: i32, month: u32, day: u32, hr: u32, min: u32, sec: u32) -> String {
        let naive_datetime = chrono::NaiveDateTime::new(
            chrono::NaiveDate::from_ymd(year, month, day),
            chrono::NaiveTime::from_hms(hr, min, sec),
        );

        let datetime: DateTime<Utc> = DateTime::from_utc(naive_datetime, Utc);
        let timestamp = datetime.format("%Y-%m-%dT%H:%M:%S.%f").to_string();
        let scheduled_time = format!("{}Z", &timestamp[..23]);
        scheduled_time
    }
}
