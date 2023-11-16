use pddl_ish_parser::{models::problem::Problem, parser::problem_parser::parse_problem};

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
        objects: vec!["website-url", "all-hyperlinks", "ai-news-links"]
            .into_iter()
            .map(String::from)
            .collect(),
        init: vec!["website-known website-url"]
            .into_iter()
            .map(String::from)
            .collect(),
        goal: vec![
            "and",
            "(all-links-extracted website-url all-hyperlinks)",
            "(relevant-links-found all-hyperlinks ai-news-links)",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        actions: vec![], // You need to define the expected actions here
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
