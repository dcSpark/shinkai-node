use pddl_ish_parser::{models::problem::Problem, parser::{problem_parser::parse_problem, object::Object, parameter::Parameter, action::Action}};

#[test]
fn test_parse_pddl_problem() {
    let input = r#"(define (problem find-ai-news)
        (:domain web-processing)
        (:objects
            website-url - url
            all-hyperlinks - links
            ai-news-links - links
        )

        (:init
            (website-known website-url)
        )

        (:goal
            (and
                (all-links-extracted website-url all-hyperlinks)
                (relevant-links-found all-hyperlinks ai-news-links)
            )
        )

        (:action extract-html
            :parameters (?url - url)
            :precondition (website-known ?url)
            :effect (html-content-available ?url)
        )

        (:action extract-links
            :parameters (?url - url)
            :precondition (html-content-available ?url)
            :effect (all-links-extracted ?url all-hyperlinks)
        )

        (:action summarize-and-filter-links
            :parameters (?links - links)
            :precondition (all-links-extracted website-url ?links)
            :effect (relevant-links-found ?links ai-news-links)
        )
    )"#;

    let expected = Problem {
        name: "find-ai-news".to_string(),
        domain: "web-processing".to_string(),
        objects: vec![
            Object {
                name: "website-url".to_string(),
                object_type: "url".to_string(),
            },
            Object {
                name: "all-hyperlinks".to_string(),
                object_type: "links".to_string(),
            },
            Object {
                name: "ai-news-links".to_string(),
                object_type: "links".to_string(),
            },
        ],
        init: vec![],
        goal: vec![],
        actions: vec![
            Action {
                name: "extract-html".to_string(),
                parameters: vec![
                    Parameter {
                        name: "url".to_string(),
                        param_type: "url".to_string(),
                    },
                ],
                preconditions: vec!["(website-known ?url)".to_string()],
                effects: vec!["(html-content-available ?url)".to_string()],
            },
            Action {
                name: "extract-links".to_string(),
                parameters: vec![
                    Parameter {
                        name: "url".to_string(),
                        param_type: "url".to_string(),
                    },
                ],
                preconditions: vec!["(html-content-available ?url)".to_string()],
                effects: vec!["(all-links-extracted ?url all-hyperlinks)".to_string()],
            },
            Action {
                name: "summarize-and-filter-links".to_string(),
                parameters: vec![
                    Parameter {
                        name: "links".to_string(),
                        param_type: "links".to_string(),
                    },
                ],
                preconditions: vec!["(all-links-extracted website-url ?links)".to_string()],
                effects: vec!["(relevant-links-found ?links ai-news-links)".to_string()],
            },
        ],
    };

    let result = parse_problem(input);
    match result {
        Ok((remaining_input, parsed_problem)) => {
            println!("{:?}", parsed_problem);
            assert_eq!(parsed_problem, expected);
        }
        Err(e) => {
            println!("Error parsing problem: {:?}", e);
            assert!(false, "Parsing failed");
        }
    }
}
