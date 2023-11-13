#[cfg(test)]
mod tests {
    use super::*;
    use futures::Future;
    use pddl_parser::domain::action::Action;
    use pddl_parser::domain::domain::Domain;
    use pddl_parser::domain::typed_parameter::TypedParameter;
    use pddl_parser::domain::typing::Type;
    use pddl_parser::error::ParserError;
    use std::{io::Cursor, path::PathBuf};
    use std::{pin::Pin, sync::Arc};
    use tokio::sync::Mutex;

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
        let res = Domain::parse(DOMAIN_PDDL.into());
        match res {
            Ok(domain) => {
                println!("Parsed domain: {:?}", domain);
                if let Some(first_action) = domain.actions.get(0) {
                    println!("\n\n First action: {:?}", first_action);
                } else {
                    println!("No actions found in the domain.");
                }
            }
            Err(e) => match e {
                ParserError::UnsupportedRequirement(_) => {}
                _ => panic!("Error with error: {:?}", e),
            },
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
        let res = Domain::parse(DOMAIN_PDDL.into());
        let mut state = SharedState::default();
        match res {
            Ok(domain) => {
                for (i, action) in domain.actions.iter().enumerate() {
                    if i == 0 {
                        // Pass a dummy parameter to the first action
                        let mut parameters = action.parameters.clone();
                        parameters.push(TypedParameter {
                            name: "https://news.ycombinator.com".to_string(),
                            type_: Type::Simple("url".to_string()),
                        });
                        state.html_fetched = fetch_html(&parameters);
                    } else {
                        execute_action(action, &mut state);
                    }
                }
            }
            Err(e) => match e {
                ParserError::UnsupportedRequirement(_) => {}
                _ => panic!("Error with error: {:?}", e),
            },
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

    fn fetch_html(_parameters: &[TypedParameter]) -> Option<String> {
        println!("Fetching HTML... {:?}", _parameters);
        Some("HTML content".to_string())
    }

    fn extract_links(_parameters: &[TypedParameter], html: &Option<String>) -> Option<Vec<String>> {
        println!("Extracting links from HTML... {:?}", html);
        Some(vec!["Link1".to_string(), "Link2".to_string()])
    }

    fn fetch_content(_parameters: &[TypedParameter], links: &Option<Vec<String>>) -> Option<String> {
        println!("Fetching content from links... {:?}", links);
        Some("Content".to_string())
    }

    fn generate_summary(_parameters: &[TypedParameter], content: &Option<String>) -> Option<String> {
        println!("Generating summary from content... {:?}", content);
        Some("Summary".to_string())
    }
}
