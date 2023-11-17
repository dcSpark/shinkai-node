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
                        preconditions: vec!["(and\n                            (has-html website html_content)\n                            (not (toolkit-ready agent))\n                          )".to_string()],
                        effects: vec!["(and\n                        (has-links html_content links)\n                        (toolkit-ready agent)\n                    )".to_string()],
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
                        preconditions: vec!["(and\n                            (has-links html_content links)\n                            (toolkit-ready agent)\n                          )".to_string()],
                        effects: vec!["(and\n                        (has-ai-news-summaries summaries)\n                        (toolkit-ready agent)\n                    )".to_string()]
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
fn test_parse_robot_cleanup_pddl_problem() {
    let input = r#"(define (problem robot-cleanup-task)
        ; This is the problem definition for a robot performing cleanup tasks
        (:domain robot-cleanup)
        ; The domain 'robot-cleanup' defines the general rules and actions for robot cleanup tasks.
        (:objects
            room1 - room
            trash1 - trash
            robot1 - robot
            ; Objects are defined with their types. We have one room, one trash object, and one robot.
        )
        (:init
            (at robot1 room1)
            (trash-in trash1 room1)
            (clean room1)
            ; The initial state: robot is in room1, trash1 is in room1, and room1 is clean.
        )
        (:goal
            (clean room1)
            ; The goal is to have the room clean.
        )
        (:action move
            :parameters (?r - robot ?from - room ?to - room)
            :precondition (at ?r ?from)
            :effect (and (not (at ?r ?from)) (at ?r ?to))
            ; Action for moving the robot from one room to another.
        )
        (:action pick-up
            :parameters (?r - robot ?t - trash ?room - room)
            :precondition (and (at ?r ?room) (trash-in ?t ?room))
            :effect (and (not (trash-in ?t ?room)) (holding ?r ?t))
            ; Action for picking up trash in the same room as the robot.
        )
        (:action dispose-of
            :parameters (?r - robot ?t - trash)
            :precondition (holding ?r ?t)
            :effect (and (not (holding ?r ?t)) (clean (room-of ?t)))
            ; Action for disposing of the trash, which results in a clean room.
        )
    )"#;

    let expected = Problem {
        name: "robot-cleanup-task".to_string(),
        domain: "robot-cleanup".to_string(),
        objects: vec![
            Object {
                name: "room1".to_string(),
                object_type: "room".to_string(),
            },
            Object {
                name: "trash1".to_string(),
                object_type: "trash".to_string(),
            },
            Object {
                name: "robot1".to_string(),
                object_type: "robot".to_string(),
            },
        ],
        init: vec![],
        goal: vec![],
        actions: vec![
            Action {
                name: "move".to_string(),
                parameters: vec![
                    Parameter {
                        name: "r".to_string(),
                        param_type: "robot".to_string(),
                    },
                    Parameter {
                        name: "from".to_string(),
                        param_type: "room".to_string(),
                    },
                    Parameter {
                        name: "to".to_string(),
                        param_type: "room".to_string(),
                    },
                ],
                preconditions: vec!["(at ?r ?from)".to_string()],
                effects: vec!["(and (not (at ?r ?from)) (at ?r ?to))".to_string()],
            },
            Action {
                name: "pick-up".to_string(),
                parameters: vec![
                    Parameter {
                        name: "r".to_string(),
                        param_type: "robot".to_string(),
                    },
                    Parameter {
                        name: "t".to_string(),
                        param_type: "trash".to_string(),
                    },
                    Parameter {
                        name: "room".to_string(),
                        param_type: "room".to_string(),
                    },
                ],
                preconditions: vec!["(and (at ?r ?room) (trash-in ?t ?room))".to_string()],
                effects: vec!["(and (not (trash-in ?t ?room)) (holding ?r ?t))".to_string()],
            },
            Action {
                name: "dispose-of".to_string(),
                parameters: vec![
                    Parameter {
                        name: "r".to_string(),
                        param_type: "robot".to_string(),
                    },
                    Parameter {
                        name: "t".to_string(),
                        param_type: "trash".to_string(),
                    },
                ],
                preconditions: vec!["(holding ?r ?t)".to_string()],
                effects: vec!["(and (not (holding ?r ?t)) (clean (room-of ?t)))".to_string()],
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
fn test_parse_news_extraction_pddl_problem() {
    let input = r#"(define (problem find-ai-news)
        (:domain news-extraction)
        (:objects
            website - website
            content - text
            news_link - hyperlink
        )

        (:init
            (website-content-fetched website)
            (all-hyperlinks-extracted website)
        )

        (:goal
            (and
                (ai-news-summarized news_link)
            )
        )
    )"#;

    let expected = Problem {
        name: "find-ai-news".to_string(),
        domain: "news-extraction".to_string(),
        objects: vec![
            Object {
                name: "website".to_string(),
                object_type: "website".to_string(),
            },
            Object {
                name: "content".to_string(),
                object_type: "text".to_string(),
            },
            Object {
                name: "news_link".to_string(),
                object_type: "hyperlink".to_string(),
            },
        ],
        init: vec![],
        goal: vec![],
        actions: vec![],
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