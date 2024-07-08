use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use crate::db::{db_errors::ShinkaiDBError, ShinkaiDB};

pub fn get_static_workflows() -> Vec<Workflow> {
    vec![
        Workflow {
            name: "example_workflow_1".to_string(),
            version: "1.0".to_string(),
            steps: vec![],
            raw: "example raw data 1".to_string(),
            description: None,
        },
        Workflow {
            name: "example_workflow_2".to_string(),
            version: "1.0".to_string(),
            steps: vec![],
            raw: "example raw data 2".to_string(),
            description: None,
        },
        // Add more workflows as needed
    ]
}

pub fn save_static_workflows(db: &ShinkaiDB, profile: ShinkaiName) -> Result<(), ShinkaiDBError> {
    let workflows = get_static_workflows();
    for workflow in workflows {
        db.save_workflow(workflow, profile.clone())?;
    }
    Ok(())
}