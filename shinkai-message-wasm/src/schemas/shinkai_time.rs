use chrono::Utc;

pub struct ShinkaiTime {}

impl ShinkaiTime {
    pub fn generate_time_now() -> String {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S.%f").to_string();
        let scheduled_time = format!("{}Z", &timestamp[..23]);
        scheduled_time
    }
}
