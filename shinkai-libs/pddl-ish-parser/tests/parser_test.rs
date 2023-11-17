use pddl_ish_parser::parser::{
    action::{parse_actions, Action},
    parameter::Parameter,
};

#[cfg(test)]
mod tests {

    use pddl_ish_parser::parser::{
        effect::extract_effects,
        parameter::parse_parameters,
        precondition::{extract_preconditions, parse_preconditions},
    };

    use super::*;

    #[test]
    fn test_single_parameter() {
        let input = "(?url - url)";
        let result = parse_parameters(input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "url");
        assert_eq!(result[0].param_type, "url");
    }

    #[test]
    fn test_multiple_parameters() {
        let input = "(?from - location ?to - location)";
        let result = parse_parameters(input).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "from");
        assert_eq!(result[0].param_type, "location");
        assert_eq!(result[1].name, "to");
        assert_eq!(result[1].param_type, "location");
    }

    #[test]
    fn test_parse_valid_action() {
        let input = r#"
            (:action move
                :parameters (?from - location ?to - location)
                :precondition (and (at ?from) (connected ?from ?to))
                :effect (and (not (at ?from)) (at ?to))
            )
        "#;

        let parse_result = parse_actions(input);
        println!("Parse result: {:?}", parse_result);

        let expected = Action {
            name: "move".to_string(),
            parameters: vec![
                Parameter {
                    name: "from".to_string(),
                    param_type: "location".to_string(),
                },
                Parameter {
                    name: "to".to_string(),
                    param_type: "location".to_string(),
                },
            ],
            preconditions: vec!["(and (at ?from) (connected ?from ?to))".to_string()],
            effects: vec!["(and (not (at ?from)) (at ?to))".to_string()],
        };

        assert_eq!(parse_result.unwrap().1[0], expected);
    }

    #[test]
    fn test_parse_action_no_parameters() {
        let input = r#"
            (:action move
                :parameters ()
                :precondition (and (clear ?x) (on-table ?x) (handempty))
                :effect (and (not (clear ?x)) (not (on-table ?x)) (not (handempty)) (holding ?x))
            )
        "#;
        let expected = Action {
            name: "move".to_string(),
            parameters: vec![],
            preconditions: vec!["(and (clear ?x) (on-table ?x) (handempty))".to_string()],
            effects: vec!["(and (not (clear ?x)) (not (on-table ?x)) (not (handempty)) (holding ?x))".to_string()],
        };

        assert_eq!(parse_actions(input).unwrap().1[0], expected);
    }

    #[test]
    fn test_parse_action_no_preconditions() {
        let input = r#"
            (:action stack
                :parameters (?x - block ?y - block)
                :precondition ()
                :effect (and (clear ?y) (not (clear ?x)) (on ?x ?y) (not (holding ?x)) (handempty))
            )
        "#;
        let expected = Action {
            name: "stack".to_string(),
            parameters: vec![
                Parameter {
                    name: "x".to_string(),
                    param_type: "block".to_string(),
                },
                Parameter {
                    name: "y".to_string(),
                    param_type: "block".to_string(),
                },
            ],
            preconditions: vec![],
            effects: vec!["(and (clear ?y) (not (clear ?x)) (on ?x ?y) (not (holding ?x)) (handempty))".to_string()],
        };

        assert_eq!(parse_actions(input).unwrap().1[0], expected);
    }

    #[test]
    fn test_parse_parameter() {
        let input = "(?url - url)";
        let expected = vec![Parameter {
            name: "url".to_string(),
            param_type: "url".to_string(),
        }];

        assert_eq!(parse_parameters(input).unwrap(), expected);
    }

    #[test]
    fn test_parse_parameters() {
        let input = "(?url - url ?another - another)";
        let expected = vec![
            Parameter {
                name: "url".to_string(),
                param_type: "url".to_string(),
            },
            Parameter {
                name: "another".to_string(),
                param_type: "another".to_string(),
            },
        ];

        assert_eq!(parse_parameters(input).unwrap(), expected);
    }

    #[test]
    fn test_parse_precondition() {
        let input = "(website-known ?url)";
        let expected = vec!["(website-known ?url)".to_string()];

        assert_eq!(parse_preconditions(input).unwrap(), expected);
    }

    #[test]
    fn test_parse_preconditions() {
        let input = "(website-known ?url another-precondition ?another)";
        let expected = vec!["(website-known ?url another-precondition ?another)".to_string()];

        assert_eq!(parse_preconditions(input).unwrap(), expected);
    }

    #[test]
    fn test_extract_preconditions() {
        let input = r#":precondition (and
                            (has-url website)
                            (toolkit-ready agent)
                          )"#;
        let expected = "(and\n                            (has-url website)\n                            (toolkit-ready agent)\n                          )";

        let result = extract_preconditions(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_extract_effects() {
        let input = r#":effect (and
                        (not (toolkit-ready agent))
                        (has-html website html_content)
                    )"#;
        let expected = "(and\n                        (not (toolkit-ready agent))\n                        (has-html website html_content)\n                    )";

        let result = extract_effects(input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_extract_html_action() {
        let input = r#"(:action extract-html
            :parameters (?url - url)
            :precondition (website-known ?url)
            :effect (html-content-available ?url)
        )
    "#;

        let expected = Action {
            name: "extract-html".to_string(),
            parameters: vec![Parameter {
                name: "url".to_string(),
                param_type: "url".to_string(),
            }],
            preconditions: vec!["(website-known ?url)".to_string()],
            effects: vec!["(html-content-available ?url)".to_string()],
        };

        // eprintln!("Parsing input: {:?}", parse_actions(input));
        assert_eq!(parse_actions(input).unwrap().1[0], expected);
    }

    #[test]
    fn test_parse_multiple_actions() {
        let input = r#"
        (:action extract-html
            :parameters (?url - url)
            :precondition (website-known ?url)
            :effect (html-content-available ?url)
        )
    
        (:action extract-links
            :parameters (?url - url)
            :effect (all-links-extracted ?url all-hyperlinks)
        )
    
        (:action summarize-and-filter-links
            :parameters (?links - links)
            :precondition (all-links-extracted website-url ?links)
            :effect (relevant-links-found ?links ai-news-links)
        )
    "#;

        let parse_result = parse_actions(input).unwrap();

        let expected = vec![
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
                preconditions: vec![],
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
        ];

        assert_eq!(parse_result.1, expected);
    }
}
