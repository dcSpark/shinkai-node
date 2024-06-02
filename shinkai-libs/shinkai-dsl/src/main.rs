extern crate pest;
extern crate pest_derive;
extern crate serde;
extern crate serde_json;

use crate::pest::Parser;
use shinkai_dsl::{
    dsl_schemas::{Rule, WorkflowParser},
    parser::parse_workflow,
};

fn main() {
    let dsl_input = r#"workflow MyProcess v1.0 {
        step Initialize {
            $R1 = 5
            $R2 = 10
        }
        step Compute {
            if $R1 < $R2 {
                call compute_difference($R2, $R1)
            }
        }
        step Finalize {
            call finalize_process($R1, $R2)
        }
    }"#;

    // Parse DSL using Pest
    let parse_result = WorkflowParser::parse(Rule::workflow, dsl_input);
    match parse_result {
        Ok(pairs) => {
            for pair in pairs {
                println!("{:?}", pair);
            }
        }
        Err(e) => eprintln!("Failed to parse DSL: {}", e),
    }

    eprintln!("\n\n\n");
    
    let workflow = parse_workflow(dsl_input).expect("Failed to parse workflow");
    println!("{:?}", workflow);
}
