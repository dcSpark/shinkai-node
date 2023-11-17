use pddl_ish_parser::{
    models::problem::Problem,
    parser::{action::Action, object::Object, parameter::Parameter, problem_parser::parse_problem},
};

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
                parameters: vec![Parameter {
                    name: "url".to_string(),
                    param_type: "url".to_string(),
                }],
                preconditions: vec!["(website-known ?url)".to_string()],
                effects: vec!["(html-content-available ?url)".to_string()],
            },
            Action {
                name: "extract-links".to_string(),
                parameters: vec![Parameter {
                    name: "url".to_string(),
                    param_type: "url".to_string(),
                }],
                preconditions: vec!["(html-content-available ?url)".to_string()],
                effects: vec!["(all-links-extracted ?url all-hyperlinks)".to_string()],
            },
            Action {
                name: "summarize-and-filter-links".to_string(),
                parameters: vec![Parameter {
                    name: "links".to_string(),
                    param_type: "links".to_string(),
                }],
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

#[test]
fn test_parse_news_finding_pddl_problem() {
    let input = r#"(define (problem find-ai-news)
        (:domain news-finding)
        (:objects
            agent - toolkit
            website - url
            html_content - html
            links - hyperlink_list
            summaries - content_list
        )

        (:init
            (has-url website)
            (toolkit-ready agent)
        )

        (:goal
            (has-ai-news-summaries summaries)
        )

        (:action fetch-html
            :parameters (agent - toolkit website - url)
            :precondition (and
                            (has-url website)
                            (toolkit-ready agent)
                          )
            :effect (and
                        (not (toolkit-ready agent))
                        (has-html website html_content)
                    )
        )

        (:action extract-links
            :parameters (agent - toolkit html_content - html)
            :precondition (and
                            (has-html website html_content)
                            (not (toolkit-ready agent))
                          )
            :effect (and
                        (has-links html_content links)
                        (toolkit-ready agent)
                    )
        )

        (:action summarize-content
            :parameters (agent - toolkit links - hyperlink_list)
            :precondition (and
                            (has-links html_content links)
                            (toolkit-ready agent)
                          )
            :effect (and
                        (has-ai-news-summaries summaries)
                        (toolkit-ready agent)
                    )
        )
    )"#;

    let expected = Problem {
        name: "find-ai-news".to_string(),
        domain: "news-finding".to_string(),
        objects: vec![
            Object {
                name: "agent".to_string(),
                object_type: "toolkit".to_string(),
            },
            Object {
                name: "website".to_string(),
                object_type: "url".to_string(),
            },
            Object {
                name: "html_content".to_string(),
                object_type: "html".to_string(),
            },
            Object {
                name: "links".to_string(),
                object_type: "hyperlink_list".to_string(),
            },
            Object {
                name: "summaries".to_string(),
                object_type: "content_list".to_string(),
            },
        ],
        init: vec![],
        goal: vec![],
        actions: vec![
            Action {
                name: "fetch-html".to_string(),
                parameters: vec![
                    Parameter {
                        name: "agent".to_string(),
                        param_type: "toolkit".to_string(),
                    },
                    Parameter {
                        name: "website".to_string(),
                        param_type: "url".to_string(),
                    },
                ],
                preconditions: vec!["(and\n                            (has-url website)\n                            (toolkit-ready agent)\n                          )".to_string()],
                effects: vec!["(and\n                        (not (toolkit-ready agent))\n                        (has-html website html_content)\n                    )".to_string()],
            },
            Action {
                name: "extract-links".to_string(),
                parameters: vec![
                    Parameter {
                        name: "agent".to_string(),
                        param_type: "toolkit".to_string(),
                    },
                    Parameter {
                        name: "html_content".to_string(),
                        param_type: "html".to_string(),
                    },
                ],
                preconditions: vec!["(and (has-html website html_content) (not (toolkit-ready agent)))".to_string()],
                effects: vec!["(and (has-links html_content links) (toolkit-ready agent))".to_string()],
            },
            Action {
                name: "summarize-content".to_string(),
                parameters: vec![
                    Parameter {
                        name: "agent".to_string(),
                        param_type: "toolkit".to_string(),
                    },
                    Parameter {
                        name: "links".to_string(),
                        param_type: "hyperlink_list".to_string(),
                    },
                ],
                preconditions: vec!["(and (has-links html_content links) (toolkit-ready agent))".to_string()],
                effects: vec!["(and (has-ai-news-summaries summaries) (toolkit-ready agent))".to_string()],
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
