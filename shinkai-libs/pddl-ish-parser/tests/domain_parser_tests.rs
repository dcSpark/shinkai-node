#[cfg(test)]
mod tests {
    use super::*;
    use pddl_ish_parser::{
        models::parser_error::ParserError,
        parser::{
            domain_type::{parse_domain_types, DomainType},
            error_context::get_error_context, predicate::{Predicate, parse_predicates, parse_predicate_line}, parameter::Parameter,
        },
    };

    #[test]
    fn test_parse_predicate_line_two_parameters() {
        let line = "(predicate-name ?param1 - type1 ?param2 - type2)";
        let expected = Some(Predicate {
            name: "predicate-name".to_string(),
            parameters: vec![
                Parameter {
                    name: "?param1".to_string(),
                    param_type: "type1".to_string(),
                },
                Parameter {
                    name: "?param2".to_string(),
                    param_type: "type2".to_string(),
                },
            ],
        });

        assert_eq!(parse_predicate_line(line), expected);
    }

    #[test]
    fn test_parse_predicate_line_three_parameters_relevant_links_found() {
        let line = "(relevant-links-found ?links - links ?ai-news-links - links)";
        let expected = Some(Predicate {
            name: "relevant-links-found".to_string(),
            parameters: vec![
                Parameter {
                    name: "?links".to_string(),
                    param_type: "links".to_string(),
                },
                Parameter {
                    name: "?ai-news-links".to_string(),
                    param_type: "links".to_string(),
                },
            ],
        });
    
        assert_eq!(parse_predicate_line(line), expected);
    }

    #[test]
    fn test_parse_domain_types() {
        let input = r#"(define (domain web-processing)
            (:requirements :strips :typing)
            (:types super-url mega_links)

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

        let expected = vec![
            DomainType {
                name: "super-url".to_string(),
            },
            DomainType {
                name: "mega_links".to_string(),
            },
        ];

        let result = parse_domain_types(input);
        assert_eq!(result.map(|(_, types)| types), Ok(expected));
    }

    #[test]
    fn test_parse_predicates() {
        let input = r#"
            (:predicates
                (website-known ?url - url)
                (html-content-available ?url - url)
                (all-links-extracted ?url - url ?links - links)
                (relevant-links-found ?links - links ?ai-news-links - links)
            )
        "#;

        let expected = vec![
            Predicate {
                name: "website-known".to_string(),
                parameters: vec![Parameter {
                    name: "?url".to_string(),
                    param_type: "url".to_string(),
                }],
            },
            Predicate {
                name: "html-content-available".to_string(),
                parameters: vec![Parameter {
                    name: "?url".to_string(),
                    param_type: "url".to_string(),
                }],
            },
            Predicate {
                name: "all-links-extracted".to_string(),
                parameters: vec![
                    Parameter {
                        name: "?url".to_string(),
                        param_type: "url".to_string(),
                    },
                    Parameter {
                        name: "?links".to_string(),
                        param_type: "links".to_string(),
                    },
                ],
            },
            Predicate {
                name: "relevant-links-found".to_string(),
                parameters: vec![
                    Parameter {
                        name: "?links".to_string(),
                        param_type: "links".to_string(),
                    },
                    Parameter {
                        name: "?ai-news-links".to_string(),
                        param_type: "links".to_string(),
                    },
                ],
            },
        ];

        let result = parse_predicates(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_domain_types_no_types() {
        let input = r#"(define (domain web-processing)
            (:requirements :strips :typing)

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

        let expected = Err(ParserError {
            description: "Failed to parse domain types".to_string(),
            code: get_error_context(input),
        });

        let result = parse_domain_types(input);
        assert_eq!(result, expected);
    }
}
