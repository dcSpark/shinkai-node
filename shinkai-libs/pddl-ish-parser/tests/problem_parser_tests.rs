#[cfg(test)]
mod tests {
    use pddl_ish_parser::parser::{object::Object, problem_parser::{parse_objects, parse_domain}};

    use super::*;

    #[test]
    fn test_parse_objects() {
        let input = "(:objects
            website-url - url
            all-hyperlinks - links
            ai-news-links - links
        )";
        let expected = vec![
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
        ];
        let result = parse_objects(input).unwrap().1;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_domain() {
        let input = "    (:domain web-processing)\n    (:objects ...";
        let expected = "web-processing".to_string();
        let (_, domain) = parse_domain(input).unwrap();
        assert_eq!(domain, expected);
    }

    #[test]
    fn test_parse_objects_various_styles() {
        let inputs = [
            r#"(:objects
                website-url - url
                all-hyperlinks - links
                ai-news-links - links
            )"#,
            r#"(:objects
                website-url - url
                all-hyperlinks - links
                ai-news-links - links)"#,
        ];

        let expected = vec![
            "website-url - url".to_string(),
            "all-hyperlinks - links".to_string(),
            "ai-news-links - links".to_string(),
        ];

        for input in &inputs {
            let result = parse_objects(input);
            match result {
                Ok((remaining_input, objects)) => {
                    // assert_eq!(objects, expected);
                    assert_eq!(remaining_input, "");
                }
                Err(e) => {
                    panic!("Error parsing objects: {:?}", e);
                }
            }
        }
    }

    #[test]
    fn test_parse_objects_complex() {
        let input = r#"(:objects
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
    )"#;

        let expected = vec![
            "website-url - url".to_string(),
            "all-hyperlinks - links".to_string(),
            "ai-news-links - links".to_string(),
        ];

        let result = parse_objects(input);
        match result {
            Ok((remaining_input, objects)) => {
                // assert_eq!(objects, expected);
                // assert remaining_input is as expected
            }
            Err(e) => {
                panic!("Error parsing objects: {:?}", e);
            }
        }
    }

}