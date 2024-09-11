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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_stopper() {
        let stopper = LLMStopper::new();
        let job_id = "test_job";

        // Test initial state
        assert!(!stopper.should_stop(job_id));

        // Test stop
        stopper.stop(job_id);
        assert!(stopper.should_stop(job_id));

        // Test reset
        stopper.reset(job_id);
        assert!(!stopper.should_stop(job_id));

        // Test non-existent key
        assert!(!stopper.should_stop("non_existent_job"));
    }
}
