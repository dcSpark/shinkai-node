#[cfg(test)]
mod tests {
    use super::*;
    use futures::Future;
    use pddl_rs::{
        compiler::compile_problem,
        parser::{parse_domain, parse_problem},
        Error, Sources,
    };

    use ariadne::{Color, Label, Report, ReportKind, Source};
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


    fn print_pddl_error(input: &str, error: &Error) {
        eprintln!("Error parsing PDDL: {:?}", error);
    }

    #[test]
    fn test_ai_news_summary_pddl() {
        // Create Sources instance from strings
        let sources = Sources {
            domain_path: "embedded_domain".into(),
            problem_path: "embedded_problem".into(),
            domain_ad: Source::from(DOMAIN_PDDL.to_string()),
            problem_ad: Source::from(PROBLEM_PDDL.to_string()),
            domain_src: DOMAIN_PDDL.to_string(),
            problem_src: PROBLEM_PDDL.to_string(),
        };

        // Parse domain
        let domain_result = parse_domain(&sources.domain_src);
        if let Err(ref e) = domain_result {
            print_pddl_error(&sources.domain_src, e);
        }

        match parse_domain(&sources.domain_src) {
            Ok(domain) => match parse_problem(&sources.problem_src, domain.requirements) {
                Ok(problem) => match compile_problem(&domain, &problem) {
                    Ok(_c_problem) => {
                        eprintln!("CompiledProblem: {:?}", _c_problem);
                    }
                    Err(e) => eprintln!("Error compiling problem inside : {:?}", e),
                },
                Err(e) => eprintln!("Error parsing problem inside: {:?}", e),
            },
            Err(e) => eprintln!("Error parsing domain inside: {:?}", e),
        }
    }
}
