use chrono::{DateTime, Utc};
pub struct ShinkaiTime {}

impl ShinkaiTime {
    pub fn generate_time_now() -> String {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S.%f").to_string();
        let scheduled_time = format!("{}Z", &timestamp[..23]);
        scheduled_time
    }

    /// Validates that the provided &str is an RFC3339 datetime
    pub fn validate_datetime_string(datetime_str: &str) -> bool {
        match DateTime::parse_from_rfc3339(datetime_str) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub fn generate_time_in_future_with_secs(secs: i64) -> String {
        let timestamp = (Utc::now() + chrono::Duration::seconds(secs))
            .format("%Y-%m-%dT%H:%M:%S.%f")
            .to_string();
        let scheduled_time = format!("{}Z", &timestamp[..23]);
        scheduled_time
    }

    pub fn generate_specific_time(year: i32, month: u32, day: u32, hr: u32, min: u32, sec: u32) -> String {
        let naive_datetime = chrono::NaiveDateTime::new(
            chrono::NaiveDate::from_ymd(year, month, day),
            chrono::NaiveTime::from_hms(hr, min, sec),
        );

        let datetime: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive_datetime, Utc);
        let timestamp = datetime.format("%Y-%m-%dT%H:%M:%S.%f").to_string();
        let scheduled_time = format!("{}Z", &timestamp[..23]);
        scheduled_time
    }
}
