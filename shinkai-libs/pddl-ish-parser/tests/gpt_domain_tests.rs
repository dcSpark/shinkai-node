use pddl_ish_parser::{
    models::domain::Domain,
    parser::{
        domain_parser::parse_domain, domain_type::DomainType, parameter::Parameter, predicate::Predicate,
        problem_parser::parse_problem_domain, action::Action,
    },
};

#[test]
fn test_parse_pddl_domain() {
    let input = r#"(define (domain web-processing)
        (:requirements :strips :typing)
        (:types url links)

        (:predicates
            (website-known ?url - url)
            (html-content-available ?url - url)
            (all-links-extracted ?url - url ?links - links)
            (relevant-links-found ?links - links ?ai-news-links - links)
        )

        (:action extract-html
            :parameters (?url - url)
            :precondition (website-known ?url)
            :effect (html-content-available ?url)
        )

        (:action extract-links
            :parameters (?url - url)
            :precondition (html-content-available ?url)
            :effect (and
                     (all-links-extracted ?url all-hyperlinks)
                     (forall (?link - links)
                       (when (link-belongs-to-website ?link ?url)
                         (link-extracted ?link))
                     )
            )
        )

        (:action summarize-and-filter-links
            :parameters (?links - links)
            :precondition (and
                           (all-links-extracted website-url ?links)
                           (not (relevant-links-found ?links ai-news-links))
                          )
            :effect (relevant-links-found ?links ai-news-links)
        )
    )"#;

    let expected = Domain {
        name: "web-processing".to_string(),
        requirements: vec![":strips".to_string(), ":typing".to_string()],
        types: vec![
            DomainType {
                name: "url".to_string(),
            },
            DomainType {
                name: "links".to_string(),
            },
        ],
        predicates: vec![
            Predicate { 
                name: "website-known".to_string(), 
                parameters: [Parameter { name: "?url".to_string(), param_type: "url".to_string() }].to_vec() 
            },
            Predicate { 
                name: "html-content-available".to_string(), 
                parameters: [Parameter { name: "?url".to_string(), param_type: "url".to_string() }].to_vec() 
            },
            Predicate { 
                name: "all-links-extracted".to_string(), 
                parameters: [Parameter { name: "?url".to_string(), param_type: "url".to_string() }, Parameter { name: "?links".to_string(), param_type: "links".to_string() }].to_vec() 
            },
            Predicate { 
                name: "relevant-links-found".to_string(), 
                parameters: [Parameter { name: "?links".to_string(), param_type: "links".to_string() }, Parameter { name: "?ai-news-links".to_string(), param_type: "links".to_string() }].to_vec() 
            },
        ],
        actions: vec![
            Action { 
                name: "extract-html".to_string(), 
                parameters: [Parameter { name: "url".to_string(), param_type: "url".to_string() }].to_vec(), 
                preconditions: ["(website-known ?url)".to_string()].to_vec(), 
                effects: ["(html-content-available ?url)".to_string()].to_vec() 
            },
            Action { 
                name: "extract-links".to_string(), 
                parameters: [Parameter { name: "url".to_string(), param_type: "url".to_string() }].to_vec(), 
                preconditions: ["(html-content-available ?url)".to_string()].to_vec(), 
                effects: ["(and\n                     (all-links-extracted ?url all-hyperlinks)\n                     (forall (?link - links)\n                       (when (link-belongs-to-website ?link ?url)\n                         (link-extracted ?link))\n                     )\n            )".to_string()].to_vec() 
            },
            Action { 
                name: "summarize-and-filter-links".to_string(), 
                parameters: [Parameter { name: "links".to_string(), param_type: "links".to_string() }].to_vec(), 
                preconditions: ["(and\n                           (all-links-extracted website-url ?links)\n                           (not (relevant-links-found ?links ai-news-links))\n                          )".to_string()].to_vec(), 
                effects: ["(relevant-links-found ?links ai-news-links)".to_string()].to_vec()
            },
        ],
    };

    let result = parse_domain(input);
    match result {
        Ok((remaining_input, parsed_domain)) => {
            assert_eq!(parsed_domain, expected);
        }
        Err(e) => {
            println!("Error parsing domain: {:?}", e);
            assert!(false, "Parsing failed");
        }
    }
}
