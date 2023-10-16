use crate::agent::job_manager::JobManager;

impl JobManager {
    pub fn start_tool_execution_inference_chain(&self) -> () {
        self.analysis_phase();

        // After analysis phase, we execute the resulting execution plan
        //    let executor = PlanExecutor::new(agent, execution_plan)?;
        //    executor.execute_plan();
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
