use dashmap::DashMap;

pub type JobId = String;

pub struct LLMStopper {
    pub stop_signal: DashMap<JobId, bool>,
}

impl LLMStopper {
    pub fn new() -> Self {
        LLMStopper {
            stop_signal: DashMap::new(),
        }
    }

    pub fn stop(&self, key: &str) {
        self.stop_signal.insert(key.to_string(), true);
    }

    pub fn reset(&self, key: &str) {
        self.stop_signal.insert(key.to_string(), false);
    }

    pub fn should_stop(&self, key: &str) -> bool {
        self.stop_signal.get(key).map_or(false, |v| *v)
    }
}
