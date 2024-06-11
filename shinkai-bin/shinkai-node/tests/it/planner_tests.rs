#[cfg(test)]
mod tests {
    use pddl_ish_parser::parser::action::Action;
    use pddl_ish_parser::parser::domain_parser::parse_domain;
    use pddl_ish_parser::parser::parameter::Parameter;
    use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;

    static DOMAIN_PDDL: &str = r#"(define (domain AI-news-summary)
            (:requirements :strips :typing)
            (:types url html link content summary)

            (:predicates
                (htmlFetched ?url - url ?html - html)
                (linksExtracted ?html - html ?link - link)
                (contentFetched ?link - link ?content - content)
                (summaryGenerated ?content - content ?summary - summary)
            )

            (:action fetchHtml
                :parameters (?url - url)
                :precondition (not (htmlFetched ?url))
                :effect (htmlFetched ?url)
            )

            (:action extractLinks
                :parameters (?html - html)
                :precondition (htmlFetched ?url ?html)
                :effect (linksExtracted ?html)
            )

            (:action fetchContent
                :parameters (?link - link)
                :precondition (linksExtracted ?html ?link)
                :effect (contentFetched ?link)
            )

            (:action generateSummary
                :parameters (?content - content)
                :precondition (contentFetched ?link ?content)
                :effect (summaryGenerated ?content)
            )
    )"#;

    static PROBLEM_PDDL: &str = r#"(define (problem example)
        (:domain AI-news-summary)
        (:objects 
            myurl - url
            myhtml - html
            mylink - link
            mycontent - content
        )
        (:init 
            ; Assuming 'myurl' is the URL we start with
            ; Other objects start without any initial state
        )
        (:goal 
            (and
                (htmlFetched myurl myhtml)
                (linksExtracted myhtml mylink)
                (contentFetched mylink mycontent)
                (summaryGenerated mycontent)
            )
        )
    )"#;

    #[test]
    fn test_ai_news_summary_pddl() {
        init_default_tracing();
        let res = parse_domain(DOMAIN_PDDL);
        match res {
            Ok((_, domain)) => {
                println!("Parsed domain: {:?}", domain);
                let first_action = domain.actions.get(0);
                assert!(first_action.is_some(), "No actions found in the domain.");
                println!("\n\n First action: {:?}", first_action.unwrap());
            }
            Err(e) => panic!("Error with error: {:?}", e),
        }
    }

    #[derive(Default)]
    struct SharedState {
        html_fetched: Option<String>,
        links_extracted: Option<Vec<String>>,
        content_fetched: Option<String>,
        summary_generated: Option<String>,
    }

    #[test]
    fn test_execute_actions() {
        init_default_tracing();
        let res = parse_domain(DOMAIN_PDDL);
        let mut state = SharedState::default();
        match res {
            Ok((_, domain)) => {
                for (i, action) in domain.actions.iter().enumerate() {
                    if i == 0 {
                        // Pass a dummy parameter to the first action
                        let mut parameters = action.parameters.clone();
                        parameters.push(Parameter {
                            name: "https://news.ycombinator.com".to_string(),
                            param_type: "url".to_string(),
                        });
                        state.html_fetched = fetch_html(&parameters);
                    } else {
                        execute_action(action, &mut state);
                    }
                }
            }
            Err(e) => panic!("Error with error: {:?}", e),
        }
    }

    fn execute_action(action: &Action, state: &mut SharedState) {
        println!("Executing action: {}", action.name);
        match action.name.as_str() {
            "fetchHtml" => state.html_fetched = fetch_html(&action.parameters),
            "extractLinks" => state.links_extracted = extract_links(&action.parameters, &state.html_fetched),
            "fetchContent" => state.content_fetched = fetch_content(&action.parameters, &state.links_extracted),
            "generateSummary" => state.summary_generated = generate_summary(&action.parameters, &state.content_fetched),
            _ => panic!("Unknown action: {}", action.name),
        }
    }

    fn fetch_html(_parameters: &[Parameter]) -> Option<String> {
        println!("Fetching HTML... {:?}", _parameters);
        Some("HTML content".to_string())
    }

    fn extract_links(_parameters: &[Parameter], html: &Option<String>) -> Option<Vec<String>> {
        println!("Extracting links from HTML... {:?}", html);
        Some(vec!["Link1".to_string(), "Link2".to_string()])
    }

    fn fetch_content(_parameters: &[Parameter], links: &Option<Vec<String>>) -> Option<String> {
        println!("Fetching content from links... {:?}", links);
        Some("Content".to_string())
    }

    fn generate_summary(_parameters: &[Parameter], content: &Option<String>) -> Option<String> {
        println!("Generating summary from content... {:?}", content);
        Some("Summary".to_string())
    }
}
