use crate::agent::job_manager::AgentManager;

impl AgentManager {
    pub fn start_tool_execution_inference_chain(&self) -> () {
        self.analysis_phase();
        self.execution_phase();
        ()
    }

    fn analysis_phase(&self) -> () {
        ()
    }

    fn execution_phase(&self) -> () {
        ()
    }
}
