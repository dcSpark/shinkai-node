use chrono::{DateTime, Utc};

/// Struct for generating RFC3339 datetimes as DateTime<Utc>
pub struct ShinkaiTime {}

impl ShinkaiTime {
    /// Generates the current Datetime
    pub fn generate_time_now() -> DateTime<Utc> {
        Utc::now()
    }

    /// Generates a Datetime in the future based on number of seconds
    pub fn generate_time_in_future_with_secs(secs: i64) -> DateTime<Utc> {
        Utc::now() + chrono::Duration::seconds(secs)
    }

    /// Generates a Datetime at a specific moment in time
    pub fn generate_specific_time(year: i32, month: u32, day: u32, hr: u32, min: u32, sec: u32) -> DateTime<Utc> {
        let naive_datetime = chrono::NaiveDateTime::new(
            chrono::NaiveDate::from_ymd(year, month, day),
            chrono::NaiveTime::from_hms(hr, min, sec),
        );

        DateTime::from_utc(naive_datetime, Utc)
    }

    /// Attempts to parse a RFC3339 datetime String
    pub fn from_rfc3339_string(datetime_str: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
        DateTime::parse_from_rfc3339(datetime_str).map(|dt| dt.with_timezone(&Utc))
    }
}

/// Struct with methods for generating RFC3339 datetimes formatted as Strings
pub struct ShinkaiStringTime {}

impl ShinkaiStringTime {
    /// Generates the current datetime as a RFC339 encoded String
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

    /// Generates a datetime String in the future based on number of seconds
    pub fn generate_time_in_future_with_secs(secs: i64) -> String {
        let timestamp = (Utc::now() + chrono::Duration::seconds(secs))
            .format("%Y-%m-%dT%H:%M:%S.%f")
            .to_string();
        let scheduled_time = format!("{}Z", &timestamp[..23]);
        scheduled_time
    }

    /// Generates a datetime String in the past based on number of seconds
    pub fn generate_time_in_past_with_secs(secs: i64) -> String {
        let timestamp = (Utc::now() - chrono::Duration::seconds(secs))
            .format("%Y-%m-%dT%H:%M:%S.%f")
            .to_string();
        let scheduled_time = format!("{}Z", &timestamp[..23]);
        scheduled_time
    }

    /// Generates a datetime String at a specific moment in time
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
