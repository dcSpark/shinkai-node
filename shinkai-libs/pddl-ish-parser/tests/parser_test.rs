use pddl_ish_parser::parser::{
    action::{parse_action, Action, parse_parameters, parse_preconditions, precondition},
    parameter::Parameter,
};

#[test]
fn test_parse_valid_action() {
    let input = ":action move (param1 param2) (precond1 precond2) (effect1 effect2)";

    // Debug output to see what the parser returns
    let parse_result = parse_action(input);
    println!("Parse result: {:?}", parse_result);

    let expected = Action {
        name: "move".to_string(),
        parameters: vec![
            Parameter {
                name: "param1".to_string(),
            },
            Parameter {
                name: "param2".to_string(),
            },
        ],
        preconditions: vec!["precond1".to_string(), "precond2".to_string()],
        effects: vec!["effect1".to_string(), "effect2".to_string()],
    };

    assert_eq!(parse_result.unwrap().1, expected);
}

#[test]
fn test_parse_action_no_parameters() {
    let input = ":action move () (precond1 precond2) (effect1 effect2)";
    let expected = Action {
        name: "move".to_string(),
        parameters: vec![],
        preconditions: vec!["precond1".to_string(), "precond2".to_string()],
        effects: vec!["effect1".to_string(), "effect2".to_string()],
    };

    assert_eq!(parse_action(input).unwrap().1, expected);
}

#[test]
fn test_parse_action_no_preconditions() {
    let input = ":action move (param1 param2) () (effect1 effect2)";
    let expected = Action {
        name: "move".to_string(),
        parameters: vec![
            Parameter {
                name: "param1".to_string(),
            },
            Parameter {
                name: "param2".to_string(),
            },
        ],
        preconditions: vec![],
        effects: vec!["effect1".to_string(), "effect2".to_string()],
    };

    assert_eq!(parse_action(input).unwrap().1, expected);
}

#[test]
fn test_parse_action_no_effects() {
    let input = ":action move (param1 param2) (precond1 precond2) ()";
    let expected = Action {
        name: "move".to_string(),
        parameters: vec![
            Parameter {
                name: "param1".to_string(),
            },
            Parameter {
                name: "param2".to_string(),
            },
        ],
        preconditions: vec!["precond1".to_string(), "precond2".to_string()],
        effects: vec![],
    };

    assert_eq!(parse_action(input).unwrap().1, expected);
}

#[test]
fn test_parse_action_invalid_format() {
    let input = "invalid format";
    assert!(parse_action(input).is_err());
}

#[test]
fn test_parse_parameter() {
    let input = "(?url - url)";
    let expected = vec![
        Parameter {
            name: "?url - url".to_string(),
        },
    ];

    assert_eq!(parse_parameters(input).unwrap().1, expected);
}

#[test]
fn test_parse_parameters() {
    let input = "(?url - url ?another - another)";
    let expected = vec![
        Parameter {
            name: "?url - url".to_string(),
        },
        Parameter {
            name: "?another - another".to_string(),
        },
    ];

    assert_eq!(parse_parameters(input).unwrap().1, expected);
}

#[test]
fn test_precondition() {
    let input = "all-links-extracted website-url ?links";
    let expected = "all-links-extracted website-url ?links".to_string();

    assert_eq!(precondition(input).unwrap().1, expected);

    let input_b = "all-links-extracted ?links";
    let expected_b = "all-links-extracted ?links".to_string();

    assert_eq!(precondition(input_b).unwrap().1, expected_b);
}

#[test]
fn test_parse_precondition() {
    let input = "(website-known ?url)";
    let expected = vec![
        "website-known ?url".to_string(),
    ];

    assert_eq!(parse_preconditions(input).unwrap().1, expected);
}

#[test]
fn test_parse_preconditions() {
    let input = "(website-known ?url another-precondition ?another)";
    let expected = vec![
        "website-known ?url".to_string(),
        "another-precondition ?another".to_string(),
    ];

    assert_eq!(parse_preconditions(input).unwrap().1, expected);
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
            name: "?url - url".to_string(),
        }],
        preconditions: vec!["website-known ?url".to_string()],
        effects: vec!["html-content-available ?url".to_string()],
    };

    eprintln!("Parsing input: {:?}", parse_action(input));

    assert_eq!(parse_action(input).unwrap().1, expected);
}
