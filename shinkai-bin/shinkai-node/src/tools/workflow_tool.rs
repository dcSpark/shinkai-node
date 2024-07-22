use std::env;

use shinkai_dsl::{dsl_schemas::Workflow, parser::parse_workflow};
use shinkai_vector_resources::embeddings::Embedding;

use super::{
    argument::ToolArgument,
    workflow_static_texts::{
        AGILITY_STORY_SYSTEM, AI_SYSTEM, ANALYZE_ANSWERS_SYSTEM, ANALYZE_CLAIMS_SYSTEM, ANALYZE_DEBATE_SYSTEM,
        ANALYZE_INCIDENT_SYSTEM, ANALYZE_LOGS_SYSTEM, ANALYZE_MALWARE_SYSTEM, ANALYZE_PAPER_SYSTEM,
        ANALYZE_PATENT_SYSTEM, ANALYZE_PERSONALITY_SYSTEM, ANALYZE_PRESENTATION_SYSTEM, ANALYZE_PROSE_JSON_SYSTEM,
        ANALYZE_PROSE_PINKER_SYSTEM, ANALYZE_PROSE_SYSTEM, ANALYZE_SPIRITUAL_TEXT_SYSTEM, ANALYZE_TECH_IMPACT_SYSTEM,
        ANALYZE_THREAT_REPORT_SYSTEM, ANALYZE_THREAT_REPORT_TRENDS_SYSTEM, ANALYZE_THREAT_REPORT_TRENDS_USER,
        ANALYZE_THREAT_REPORT_USER, ANSWER_INTERVIEW_QUESTION_SYSTEM, ASK_SECURE_BY_DESIGN_QUESTIONS_SYSTEM,
        CAPTURE_THINKERS_WORK_SYSTEM, CHECK_AGREEMENT_SYSTEM, CLEAN_TEXT_SYSTEM, CODING_MASTER_SYSTEM,
        COMPARE_AND_CONTRAST_SYSTEM, CREATE_5_SENTENCE_SUMMARY_SYSTEM, CREATE_ACADEMIC_PAPER_SYSTEM,
        CREATE_AI_JOBS_ANALYSIS_SYSTEM, CREATE_APHORISMS_SYSTEM, CREATE_ART_PROMPT_SYSTEM, CREATE_BETTER_FRAME_SYSTEM,
        CREATE_CODING_PROJECT_SYSTEM, CREATE_COMMAND_SYSTEM, CREATE_CYBER_SUMMARY_SYSTEM,
        CREATE_GIT_DIFF_COMMIT_SYSTEM, CREATE_GRAPH_FROM_INPUT_SYSTEM, CREATE_HORMOZI_OFFER_SYSTEM,
        CREATE_IDEA_COMPASS_SYSTEM, CREATE_INVESTIGATION_VISUALIZATION_SYSTEM, CREATE_KEYNOTE_SYSTEM,
        CREATE_LOGO_SYSTEM, CREATE_MARKMAP_VISUALIZATION_SYSTEM, CREATE_MERMAID_VISUALIZATION_SYSTEM,
        CREATE_MICRO_SUMMARY_SYSTEM, CREATE_NETWORK_THREAT_LANDSCAPE_SYSTEM, CREATE_NETWORK_THREAT_LANDSCAPE_USER,
        CREATE_NPC_SYSTEM, CREATE_PATTERN_SYSTEM, CREATE_QUIZ_SYSTEM, CREATE_READING_PLAN_SYSTEM,
        CREATE_REPORT_FINDING_SYSTEM, CREATE_REPORT_FINDING_USER, CREATE_SECURITY_UPDATE_SYSTEM,
        CREATE_SHOW_INTRO_SYSTEM, CREATE_SIGMA_RULES_SYSTEM, CREATE_STRIDE_THREAT_MODEL_SYSTEM, CREATE_SUMMARY_SYSTEM,
        CREATE_TAGS_SYSTEM, CREATE_THREAT_SCENARIOS_SYSTEM, CREATE_UPGRADE_PACK_SYSTEM, CREATE_VIDEO_CHAPTERS_SYSTEM,
        CREATE_VISUALIZATION_SYSTEM, EXPLAIN_CODE_SYSTEM, EXPLAIN_CODE_USER, EXPLAIN_DOCS_SYSTEM,
        EXPLAIN_PROJECT_SYSTEM, EXPLAIN_TERMS_SYSTEM, EXPORT_DATA_AS_CSV_SYSTEM,
        EXTRACT_ALGORITHM_UPDATE_RECOMMENDATIONS_SYSTEM, EXTRACT_ARTICLE_WISDOM_SYSTEM, EXTRACT_ARTICLE_WISDOM_USER,
        EXTRACT_BOOK_IDEAS_SYSTEM, EXTRACT_BOOK_RECOMMENDATIONS_SYSTEM, EXTRACT_BUSINESS_IDEAS_SYSTEM,
        EXTRACT_CONTROVERSIAL_IDEAS_SYSTEM, EXTRACT_EXTRAORDINARY_CLAIMS_SYSTEM, EXTRACT_IDEAS_SYSTEM,
        EXTRACT_INSIGHTS_SYSTEM, EXTRACT_MAIN_IDEA_SYSTEM, EXTRACT_PATTERNS_SYSTEM, EXTRACT_POC_SYSTEM,
        EXTRACT_PREDICTIONS_SYSTEM, EXTRACT_QUESTIONS_SYSTEM, EXTRACT_RECOMMENDATIONS_SYSTEM,
        EXTRACT_REFERENCES_SYSTEM, EXTRACT_SONG_MEANING_SYSTEM, EXTRACT_SPONSORS_SYSTEM, EXTRACT_VIDEOID_SYSTEM,
        EXTRACT_WISDOM_AGENTS_SYSTEM, EXTRACT_WISDOM_DM_SYSTEM, EXTRACT_WISDOM_NOMETA_SYSTEM, EXTRACT_WISDOM_SYSTEM,
        FIND_HIDDEN_MESSAGE_SYSTEM, FIND_LOGICAL_FALLACIES_SYSTEM, GENERATE_QUIZ_SYSTEM, GET_WOW_PER_MINUTE_SYSTEM,
        GET_YOUTUBE_RSS_SYSTEM, IMPROVE_ACADEMIC_WRITING_SYSTEM, IMPROVE_PROMPT_SYSTEM, IMPROVE_REPORT_FINDING_SYSTEM,
        IMPROVE_REPORT_FINDING_USER, IMPROVE_WRITING_SYSTEM, LABEL_AND_RATE_SYSTEM, OFFICIAL_PATTERN_TEMPLATE_SYSTEM,
        PROVIDE_GUIDANCE_SYSTEM, RATE_AI_RESPONSE_SYSTEM, RATE_AI_RESULT_SYSTEM, RATE_CONTENT_SYSTEM,
        RATE_CONTENT_USER, RATE_VALUE_SYSTEM, RAW_QUERY_SYSTEM, RECOMMEND_ARTISTS_SYSTEM,
        SHOW_FABRIC_OPTIONS_MARKMAP_SYSTEM, SUGGEST_PATTERN_SYSTEM, SUGGEST_PATTERN_USER, SUMMARIZE_DEBATE_SYSTEM,
        SUMMARIZE_GIT_CHANGES_SYSTEM, SUMMARIZE_GIT_DIFF_SYSTEM, SUMMARIZE_LECTURE_SYSTEM,
        SUMMARIZE_LEGISLATION_SYSTEM, SUMMARIZE_MICRO_SYSTEM, SUMMARIZE_NEWSLETTER_SYSTEM, SUMMARIZE_PAPER_SYSTEM,
        SUMMARIZE_PROMPT_SYSTEM, SUMMARIZE_PULL_REQUESTS_SYSTEM, SUMMARIZE_RPG_SESSION_SYSTEM, SUMMARIZE_SYSTEM,
        TO_FLASHCARDS_SYSTEM, TWEET_SYSTEM, WRITE_ESSAY_SYSTEM, WRITE_HACKERONE_REPORT_SYSTEM,
        WRITE_MICRO_ESSAY_SYSTEM, WRITE_PULL_REQUEST_SYSTEM, WRITE_SEMGREP_RULE_SYSTEM,
    },
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowTool {
    pub workflow: Workflow,
    pub embedding: Option<Embedding>,
}

impl WorkflowTool {
    pub fn new(workflow: Workflow) -> Self {
        WorkflowTool {
            workflow,
            embedding: None,
        }
    }

    pub fn get_db_key(&self) -> String {
        format!("{}:::{}", self.workflow.name, self.workflow.version)
    }

    pub fn get_name(&self) -> String {
        self.workflow.name.clone()
    }

    pub fn get_description(&self) -> String {
        self.workflow.description.clone().unwrap_or_default()
    }

    pub fn get_input_args(&self) -> Vec<ToolArgument> {
        if self.workflow.raw.contains("$INPUT") {
            vec![ToolArgument::new(
                "input".to_string(),
                "string".to_string(),
                "Input for the workflow".to_string(),
                true,
            )]
        } else {
            Vec::new()
        }
    }

    // Additional methods that might be useful
    pub fn get_embedding(&self) -> Option<Embedding> {
        self.embedding.clone()
    }

    pub fn format_embedding_string(&self) -> String {
        let mut embedding_string = format!("{} {}\n", self.get_name(), self.get_description());
        embedding_string.push_str("Input Args:\n");

        for arg in self.get_input_args() {
            embedding_string.push_str(&format!("- {} : {}\n", arg.name, arg.description));
        }

        embedding_string
    }
}

impl WorkflowTool {
    pub fn static_tools() -> Vec<Self> {
        let is_testing = env::var("IS_TESTING")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        if is_testing {
            vec![
                Self::get_extensive_summary_workflow(),
                Self::get_hyde_inference_workflow(),
                Self::get_agility_story_workflow(),
            ]
        } else {
            vec![
                Self::get_extensive_summary_workflow(),
                Self::get_hyde_inference_workflow(),
                Self::get_agility_story_workflow(),
                Self::get_ai_workflow(),
                Self::get_analyze_answers_workflow(),
                Self::get_analyze_claims_workflow(),
                Self::get_analyze_debate_workflow(),
                Self::get_analyze_incident_workflow(),
                Self::get_analyze_logs_workflow(),
                Self::get_analyze_malware_workflow(),
                Self::get_analyze_paper_workflow(),
                Self::get_analyze_patent_workflow(),
                Self::get_analyze_personality_workflow(),
                Self::get_analyze_presentation_workflow(),
                Self::get_analyze_prose_json_workflow(),
                Self::get_analyze_prose_pinker_workflow(),
                Self::get_analyze_prose_workflow(),
                Self::get_analyze_spiritual_text_workflow(),
                Self::get_analyze_tech_impact_workflow(),
                Self::get_analyze_threat_report_trends_workflow(),
                Self::get_analyze_threat_report_workflow(),
                Self::get_answer_interview_question_workflow(),
                Self::get_ask_secure_by_design_questions_workflow(),
                Self::get_capture_thinkers_work_workflow(),
                Self::get_check_agreement_workflow(),
                Self::get_clean_text_workflow(),
                Self::get_coding_master_workflow(),
                Self::get_compare_and_contrast_workflow(),
                Self::get_create_5_sentence_summary_workflow(),
                Self::get_create_academic_paper_workflow(),
                Self::get_create_ai_jobs_analysis_workflow(),
                Self::get_create_aphorisms_workflow(),
                Self::get_create_art_prompt_workflow(),
                Self::get_create_better_frame_workflow(),
                Self::get_create_coding_project_workflow(),
                Self::get_create_command_workflow(),
                Self::get_create_cyber_summary_workflow(),
                Self::get_create_git_diff_commit_workflow(),
                Self::get_create_graph_from_input_workflow(),
                Self::get_create_hormozi_offer_workflow(),
                Self::get_create_idea_compass_workflow(),
                Self::get_create_investigation_visualization_workflow(),
                Self::get_create_keynote_workflow(),
                Self::get_create_logo_workflow(),
                Self::get_create_markmap_visualization_workflow(),
                Self::get_create_mermaid_visualization_workflow(),
                Self::get_create_micro_summary_workflow(),
                Self::get_create_network_threat_landscape_workflow(),
                Self::get_create_npc_workflow(),
                Self::get_create_pattern_workflow(),
                Self::get_create_quiz_workflow(),
                Self::get_create_reading_plan_workflow(),
                Self::get_create_report_finding_workflow(),
                Self::get_create_security_update_workflow(),
                Self::get_create_show_intro_workflow(),
                Self::get_create_sigma_rules_workflow(),
                Self::get_create_stride_threat_model_workflow(),
                Self::get_create_summary_workflow(),
                Self::get_create_tags_workflow(),
                Self::get_create_threat_scenarios_workflow(),
                Self::get_create_upgrade_pack_workflow(),
                Self::get_create_video_chapters_workflow(),
                Self::get_create_visualization_workflow(),
                Self::get_explain_code_workflow(),
                Self::get_explain_docs_workflow(),
                Self::get_explain_project_workflow(),
                Self::get_explain_terms_workflow(),
                Self::get_export_data_as_csv_workflow(),
                Self::get_extract_algorithm_update_recommendations_workflow(),
                Self::get_extract_article_wisdom_workflow(),
                Self::get_extract_book_ideas_workflow(),
                Self::get_extract_book_recommendations_workflow(),
                Self::get_extract_business_ideas_workflow(),
                Self::get_extract_controversial_ideas_workflow(),
                Self::get_extract_extraordinary_claims_workflow(),
                Self::get_extract_ideas_workflow(),
                Self::get_extract_insights_workflow(),
                Self::get_extract_main_idea_workflow(),
                Self::get_extract_patterns_workflow(),
                Self::get_extract_poc_workflow(),
                Self::get_extract_predictions_workflow(),
                Self::get_extract_questions_workflow(),
                Self::get_extract_recommendations_workflow(),
                Self::get_extract_references_workflow(),
                Self::get_extract_song_meaning_workflow(),
                Self::get_extract_sponsors_workflow(),
                Self::get_extract_videoid_workflow(),
                Self::get_extract_wisdom_agents_workflow(),
                Self::get_extract_wisdom_dm_workflow(),
                Self::get_extract_wisdom_nometa_workflow(),
                Self::get_extract_wisdom_workflow(),
                Self::get_find_hidden_message_workflow(),
                Self::get_find_logical_fallacies_workflow(),
                Self::get_generate_quiz_workflow(),
                Self::get_get_wow_per_minute_workflow(),
                Self::get_get_youtube_rss_workflow(),
                Self::get_improve_academic_writing_workflow(),
                Self::get_improve_prompt_workflow(),
                Self::get_improve_report_finding_workflow(),
                Self::get_improve_writing_workflow(),
                Self::get_label_and_rate_workflow(),
                Self::get_official_pattern_template_workflow(),
                Self::get_provide_guidance_workflow(),
                Self::get_rate_ai_response_workflow(),
                Self::get_rate_ai_result_workflow(),
                Self::get_rate_content_workflow(),
                Self::get_rate_value_workflow(),
                Self::get_raw_query_workflow(),
                Self::get_recommend_artists_workflow(),
                Self::get_show_fabric_options_markmap_workflow(),
                Self::get_suggest_pattern_workflow(),
                Self::get_summarize_debate_workflow(),
                Self::get_summarize_git_changes_workflow(),
                Self::get_summarize_git_diff_workflow(),
                Self::get_summarize_lecture_workflow(),
                Self::get_summarize_legislation_workflow(),
                Self::get_summarize_micro_workflow(),
                Self::get_summarize_newsletter_workflow(),
                Self::get_summarize_paper_workflow(),
                Self::get_summarize_prompt_workflow(),
                Self::get_summarize_pull_requests_workflow(),
                Self::get_summarize_rpg_session_workflow(),
                Self::get_summarize_workflow(),
                Self::get_to_flashcards_workflow(),
                Self::get_tweet_workflow(),
                Self::get_write_essay_workflow(),
                Self::get_write_hackerone_report_workflow(),
                Self::get_write_micro_essay_workflow(),
                Self::get_write_pull_request_workflow(),
                Self::get_write_semgrep_rule_workflow(),
            ]
        }
    }

    fn get_extensive_summary_workflow() -> Self {
        let raw_workflow = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            }
        "#;

        let mut workflow = parse_workflow(raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Reviews in depth all the content to generate a summary.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_hyde_inference_workflow() -> Self {
        let raw_workflow = r#"
            workflow HydeInference v0.1 {
                step Initialize {
                    $PROMPT = "write a passage to answer the question: "
                    $HYDE_PROMPT = call concat($PROMPT, $INPUT)
                    $HYDE_PASSAGE = call inference_no_ws($HYDE_PROMPT)
                    $HYDE_INPUT = call concat($INPUT, ". ", $HYDE_PASSAGE )
                    $EMBEDDINGS = call search_embeddings_in_job_scope($HYDE_INPUT)
                }
                step Summarize {
                    $CONNECTOR = "\nLeverage the following information to answer the previous query: --- start ---"
                    $NEW_INPUT = call concat($INPUT, $CONNECTOR, $EMBEDDINGS) 
                    $RESULT = call inference($NEW_INPUT)
                }
            }
        "#;

        let mut workflow = parse_workflow(raw_workflow).expect("Failed to parse workflow");
        workflow.description =
            Some("Generates a passage to answer a question and uses embeddings to refine the answer.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_generate_quiz_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow GenerateQuiz v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}
            "#,
            GENERATE_QUIZ_SYSTEM.replace('"', "\\\"")
        );

        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates quiz questions based on the provided learning objectives.".to_string());

        WorkflowTool::new(workflow)
    }

    // Auto-generated code
    // The prompts are from the great folks at Fabric https://github.com/fabric/fabric
    /*
    Copyright (c) 2020 Jeff Forcier.
    All rights reserved.

    Redistribution and use in source and binary forms, with or without
    modification, are permitted provided that the following conditions are met:

        * Redistributions of source code must retain the above copyright notice,
        this list of conditions and the following disclaimer.
        * Redistributions in binary form must reproduce the above copyright notice,
        this list of conditions and the following disclaimer in the documentation
        and/or other materials provided with the distribution.

    THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
    ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
    WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
    DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
    FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
    DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
    SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
    CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
    OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
    OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
     */

    fn get_agility_story_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Agility_story v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            AGILITY_STORY_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates user stories and acceptance criteria for specified topics, focusing on Agile framework principles. This prompt specializes in translating topics into structured Agile documentation, specifically for user story and acceptance criteria creation. The expected output is a JSON-formatted document detailing the topic, user story, and acceptance criteria.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_ai_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Ai v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            AI_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes and responds to questions with insightful bullet points. It involves creating a mental model of the question for deeper understanding. The output consists of 3-5 concise bullet points, each with a 10-word limit.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_answers_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_answers v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_ANSWERS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Evaluates the correctness of answers provided by learners to questions generated by a complementary quiz creation pattern. It aims to assess understanding of learning objectives and identify areas needing further study. The expected output is an analysis of the learner's answers, indicating their grasp of the subject matter.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_claims_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_claims v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_CLAIMS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes and rates the truth claims in input, providing evidence for and against, along with a balanced view. It separates truth claims from arguments, offering a nuanced analysis with ratings and labels for each claim. The output includes a summary, evidence, refutations, logical fallacies, ratings, labels, and an overall score and analysis.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_debate_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_debate v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_DEBATE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes debate transcripts to help users understand different viewpoints and broaden their perspectives. It maps out claims, analyzes them neutrally, and rates the debate's insightfulness and emotionality. The output includes scores, participant emotionality, argument summaries with sources, and lists of agreements, disagreements, misunderstandings, learnings, and takeaways.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_incident_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_incident v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_INCIDENT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes cybersecurity breach articles by extracting key information efficiently, focusing on conciseness and organization. It avoids inferential conclusions, relying solely on the article's content for details like attack date, type, and impact. The output is a structured summary with specific details about the cybersecurity incident, including attack methods, vulnerabilities, and recommendations for prevention.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_logs_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_logs v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_LOGS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes a server log file to identify patterns, anomalies, and potential issues, aiming to enhance the server's reliability and performance. The process involves a detailed examination of log entries, assessment of operational reliability, and identification of recurring issues. Recommendations for improvements are provided based on data-driven analysis, excluding personal opinions and irrelevant information.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_malware_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_malware v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_MALWARE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes malware across various platforms, focusing on extracting indicators of compromise and detailed malware behavior. This approach includes analyzing telemetry and community data to aid in malware detection and analysis. The expected output includes a summary of findings, potential indicators of compromise, Mitre Att&CK techniques, pivoting advice, detection strategies, suggested Yara rules, additional references, and technical recommendations.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_paper_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_paper v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_PAPER_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("This service analyzes research papers to determine their main findings, scientific rigor, and quality. It uniquely maps out claims, evaluates study design, and assesses conflicts of interest. The output includes a summary, author details, findings, study quality, and a final grade with explanations.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_patent_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_patent v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_PATENT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt outlines the role and responsibilities of a patent examiner, emphasizing the importance of technical and legal expertise in evaluating patents. It details the steps for examining a patent, including identifying the technology field, problem addressed, solution, advantages, novelty, and inventive step, and summarizing the core idea and keywords. The expected output involves detailed analysis and documentation in specific sections without concern for length, using bullet points for clarity.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_personality_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_personality v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_PERSONALITY_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Performs in-depth psychological analysis on the main individual in the provided input. It involves identifying the primary person, deeply contemplating their language and responses, and comparing these to known human psychology principles. The output includes a concise psychological profile summary and detailed supporting points.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_presentation_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_presentation v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_PRESENTATION_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes and critiques presentations, focusing on content, speaker's psychology, and the difference between stated and actual goals. It involves comparing intended messages to actual content, including self-references and entertainment attempts. The output includes scores and summaries for ideas, selflessness, and entertainment, plus an overall analysis.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_prose_json_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_prose_json v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_PROSE_JSON_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Evaluates the quality of writing and content, providing ratings and recommendations for improvement based on novelty, clarity, and overall messaging. It assesses ideas for their freshness and originality, clarity of argument, and quality of prose, offering a structured approach to critique. The expected output is a JSON object summarizing these evaluations and recommendations.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_prose_pinker_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_prose_pinker v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_PROSE_PINKER_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Evaluates prose based on Steven Pinker's writing principles, identifying its current style and recommending improvements for clarity and engagement. It involves analyzing the text's adherence to Pinker's stylistic categories and avoiding common pitfalls in writing. The output includes a detailed analysis of the prose's style, strengths, weaknesses, and specific examples of both effective and ineffective writing elements.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_prose_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_prose v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_PROSE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Evaluates the quality of writing by assessing its novelty, clarity, and prose, and provides improvement recommendations. It uses a detailed approach to rate each aspect on a specific scale and ensures the overall rating reflects the lowest individual score. The expected output includes ratings and concise improvement tips.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_spiritual_text_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_spiritual_text v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_SPIRITUAL_TEXT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes spiritual texts to highlight surprising claims and contrasts them with the King James Bible. This approach involves detailed comparison, providing examples from both texts to illustrate differences. The output consists of concise bullet points summarizing these findings.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_tech_impact_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_tech_impact v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANALYZE_TECH_IMPACT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes the societal impact of technology projects by breaking down their intentions, outcomes, and broader implications, including ethical considerations. It employs a structured approach, detailing the project's objectives, technologies used, target audience, outcomes, societal impact, ethical considerations, and sustainability. The expected output includes summaries, lists, and analyses across specified sections.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_threat_report_trends_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_threat_report_trends v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $CUSTOM_USER = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM, $CUSTOM_USER)
                    }}
                }}"
            "#,
            ANALYZE_THREAT_REPORT_TRENDS_SYSTEM, ANALYZE_THREAT_REPORT_TRENDS_USER
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes cybersecurity threat reports to identify up to 50 unique, surprising, and insightful trends. This process involves a deep, expert analysis to uncover new and interesting information. The expected output is a list of trends without repetition or formatting embellishments.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_analyze_threat_report_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Analyze_threat_report v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $CUSTOM_USER = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM, $CUSTOM_USER)
                    }}
                }}"
            "#,
            ANALYZE_THREAT_REPORT_SYSTEM, ANALYZE_THREAT_REPORT_USER
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt instructs a super-intelligent cybersecurity expert to analyze and extract key insights from cybersecurity threat reports. It emphasizes identifying new, interesting, and surprising information, and organizing these findings into concise, categorized summaries. The expected output includes a one-sentence summary, trends, statistics, quotes, references, and recommendations from the report, all formatted in plain language and without repetition.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_answer_interview_question_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Answer_interview_question v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ANSWER_INTERVIEW_QUESTION_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates tailored responses to technical interview questions, aiming for a casual yet insightful tone. The AI draws from a technical knowledge base and professional experiences to construct responses that demonstrate depth and alternative perspectives. Outputs are structured first-person responses, including context, main explanation, alternative approach, and evidence-based conclusion.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_ask_secure_by_design_questions_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Ask_secure_by_design_questions v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            ASK_SECURE_BY_DESIGN_QUESTIONS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates a comprehensive set of security-focused questions tailored to the fundamental design of a specific project. This process involves deep analysis and conceptualization of the project's components and their security needs. The output includes a summary and a detailed list of security questions organized by themes.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_capture_thinkers_work_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Capture_thinkers_work v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CAPTURE_THINKERS_WORK_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes teachings and philosophies of notable individuals or philosophical schools, providing detailed templates on their backgrounds, ideas, and applications. It offers a structured approach to encapsulating complex thoughts into accessible summaries. The output includes encapsulations, background information, schools of thought, impactful ideas, primary teachings, works, quotes, applications, and life advice.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_check_agreement_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Check_agreement v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CHECK_AGREEMENT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt outlines a process for analyzing contracts and agreements to identify potential issues or 'gotchas.' It involves summarizing the document, listing important aspects, categorizing issues by severity, and drafting responses for critical and important items. The expected output includes a concise summary, detailed callouts, categorized issues, and recommended responses in Markdown format.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_clean_text_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Clean_text v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CLEAN_TEXT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes and corrects formatting issues in text without altering the content. It focuses on removing odd line breaks to improve readability. The expected output is a clean, well-formatted version of the original text.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_coding_master_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Coding_master v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CODING_MASTER_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Explains coding concepts or languages to beginners, using examples from reputable sources and illustrating points with formatted code. The approach emphasizes clarity and accessibility, incorporating examples from Codeacademy and NetworkChuck. Outputs include markdown-formatted code and structured lists of ideas, recommendations, habits, facts, and insights, adhering to specific word counts.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_compare_and_contrast_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Compare_and_contrast v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            COMPARE_AND_CONTRAST_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Compares and contrasts a list of items, focusing on their differences and similarities. The approach involves analyzing the items across various topics, organizing the findings into a markdown table. The expected output is a structured comparison in table format.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_5_sentence_summary_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_5_sentence_summary v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_5_SENTENCE_SUMMARY_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates concise summaries or answers at five decreasing levels of depth. It involves deep understanding and thoughtful analysis of the input. The output is a structured list capturing the essence in 5, 4, 3, 2, and 1 word(s).".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_academic_paper_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_academic_paper v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_ACADEMIC_PAPER_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Produces high-quality, authoritative Latex academic papers with clear concept explanations. It focuses on logical layout and simplicity while maintaining a professional appearance. The expected output is LateX code formatted in a two-column layout with a header and footer.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_ai_jobs_analysis_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_ai_jobs_analysis v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_AI_JOBS_ANALYSIS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes job reports to identify roles least and most vulnerable to automation, offering strategies for enhancing job security. It leverages historical insights to predict automation's impact on various job categories. The output includes a detailed analysis and recommendations for resilience against automation.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_aphorisms_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_aphorisms v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_APHORISMS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates a list of 20 aphorisms related to the given topic(s), ensuring variety in their beginnings. It focuses on sourcing quotes from real individuals. The output includes each aphorism followed by the name of the person who said it.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_art_prompt_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_art_prompt v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_ART_PROMPT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt guides an expert artist in conceptualizing and instructing AI to create art that perfectly encapsulates a given concept. It emphasizes deep thought on the concept and its visual representation, aiming for compelling and interesting artwork. The expected output is a 100-word description that not only instructs the AI on what to create but also how the art should evoke feelings and suggest style through examples.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_better_frame_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_better_frame v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_BETTER_FRAME_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The essay explores the concept of framing as a way to construct and interpret reality through different lenses, emphasizing the power of perspective in shaping one's experience of the world. It highlights various dichotomies in perceptions around topics like AI, race/gender, success, personal identity, and control over life, illustrating how different frames can lead to vastly different outlooks and outcomes. The author argues for the importance of choosing positive frames to improve individual and collective realities, suggesting that changing frames can change outcomes and foster more positive social dynamics.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_coding_project_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_coding_project v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_CODING_PROJECT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates wireframes and starter code for coding projects based on user ideas. It specifically caters to transforming ideas into actionable project outlines and code skeletons, including detailed steps and file structures. The output includes project summaries, structured directories, and initial code setups.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_command_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_command v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_COMMAND_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates specific command lines for various penetration testing tools based on a brief description of the desired outcome. This approach leverages the tool's help documentation to ensure accuracy and relevance. The expected output is a precise command that aligns with the user's objectives for the tool.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_cyber_summary_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_cyber_summary v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_CYBER_SUMMARY_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt instructs on creating a comprehensive summary of cybersecurity threats, vulnerabilities, incidents, and malware for a technical audience. It emphasizes deep understanding through repetitive analysis and visualization techniques. The expected output includes a concise summary and categorized lists of cybersecurity issues.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_git_diff_commit_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_git_diff_commit v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_GIT_DIFF_COMMIT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("This prompt provides instructions for using specific Git commands to manage code changes. It explains how to view differences since the last commit and display the current state of the repository. The expected output is a guide on executing these commands.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_graph_from_input_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_graph_from_input v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_GRAPH_FROM_INPUT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_hormozi_offer_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_hormozi_offer v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_HORMOZI_OFFER_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_idea_compass_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_idea_compass v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_IDEA_COMPASS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Guides users in developing a structured exploration of ideas through a detailed template. It emphasizes clarity and organization by breaking down the process into specific steps, including defining, supporting, and contextualizing the idea. The expected output is a comprehensive summary with related ideas, evidence, and sources organized in a structured format.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_investigation_visualization_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_investigation_visualization v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_INVESTIGATION_VISUALIZATION_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Creates detailed GraphViz visualizations to illustrate complex intelligence investigations and data insights. This approach involves extensive analysis, organizing information, and visual representation using shapes, colors, and labels for clarity. The output includes a comprehensive diagram and analytical conclusions with a certainty rating.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_keynote_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_keynote v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_KEYNOTE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt guides in creating TED-quality keynote presentations from provided input, focusing on narrative flow and practical takeaways. It outlines steps for structuring the presentation into slides with concise bullet points, images, and speaker notes. The expected output includes a story flow, the final takeaway, and a detailed slide deck presentation.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_logo_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_logo v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_LOGO_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates simple, minimalist company logos based on provided input, focusing on elegance and impact without text. The approach emphasizes super minimalist designs. The output is a prompt for an AI image generator to create a simple, vector graphic logo.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_markmap_visualization_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_markmap_visualization v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_MARKMAP_VISUALIZATION_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Transforms complex ideas into visual formats using MarkMap syntax for easy understanding. This process involves simplifying concepts to ensure they can be effectively represented within the constraints of MarkMap. The output is a MarkMap syntax diagram that visually communicates the core ideas.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_mermaid_visualization_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_mermaid_visualization v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_MERMAID_VISUALIZATION_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Transforms complex ideas into simplified Mermaid (Markdown) visual diagrams. This process involves creating detailed visualizations that can independently explain concepts using Mermaid syntax, focusing on clarity and comprehensibility. The expected output is a Mermaid syntax diagram accompanied by a concise visual explanation.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_micro_summary_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_micro_summary v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_MICRO_SUMMARY_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes content into a Markdown formatted summary, focusing on brevity and clarity. It emphasizes creating concise, impactful points and takeaways. The output includes a one-sentence summary, main points, and key takeaways, each adhering to strict word limits.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_network_threat_landscape_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_network_threat_landscape v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $CUSTOM_USER = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM, $CUSTOM_USER)
                    }}
                }}"
            "#,
            CREATE_NETWORK_THREAT_LANDSCAPE_SYSTEM, CREATE_NETWORK_THREAT_LANDSCAPE_USER
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes open ports and services from network scans to identify security risks and provide recommendations. This process involves a detailed examination of port and service statistics to uncover potential vulnerabilities. The expected output is a markdown formatted threat report with sections on description, risk, recommendations, a concise summary, trends, and quotes from the analysis.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_npc_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_npc v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_NPC_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates detailed NPCs for D&D 5th edition, incorporating a wide range of characteristics from background to appearance. It emphasizes creativity in developing a character's backstory, traits, and goals. The output is a comprehensive character profile suitable for gameplay.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_pattern_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_pattern v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_PATTERN_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The AI assistant is designed to interpret and respond to LLM/AI prompts with structured outputs. It specializes in organizing and analyzing prompts to produce responses that adhere to specific instructions and formatting requirements. The assistant ensures accuracy and alignment with the intended outcomes through meticulous analysis.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_quiz_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_quiz v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_QUIZ_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates questions for reviewing learning objectives based on provided subject and objectives. It requires defining the subject and learning objectives for accurate question generation. The output consists of questions aimed at helping students review key concepts.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_reading_plan_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_reading_plan v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_READING_PLAN_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Designs a tailored three-phase reading plan based on user input, focusing on an author or specific guidance. It carefully selects books from various sources, including hidden gems, to enhance the user's knowledge on the topic. The output includes a concise plan summary and categorized reading lists with reasons for each selection.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_report_finding_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_report_finding v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $CUSTOM_USER = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM, $CUSTOM_USER)
                    }}
                }}"
            "#,
            CREATE_REPORT_FINDING_SYSTEM, CREATE_REPORT_FINDING_USER
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt instructs the creation of a detailed markdown security finding report, incorporating sections like Description, Risk, Recommendations, and others, based on a vulnerability title and explanation provided by the user. It emphasizes a structured, insightful approach to documenting cybersecurity vulnerabilities. The expected output is a comprehensive report with specific sections, focusing on clarity, insightfulness, and relevance to cybersecurity assessment.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_security_update_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_security_update v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_SECURITY_UPDATE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt instructs on creating concise security updates for newsletters, focusing on cybersecurity developments, threats, advisories, and new vulnerabilities. It emphasizes brevity and relevance, requiring links to further information. The expected output includes structured sections with short descriptions and relevant details, aiming to inform readers about the latest security concerns efficiently.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_show_intro_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_show_intro v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_SHOW_INTRO_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Creates compelling short intros for podcasts, focusing on the most interesting aspects of the show. It involves listening to the entire show, identifying key topics, and highlighting them in a concise introduction. The output is a structured intro that teases the conversation's main points.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_sigma_rules_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_sigma_rules v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_SIGMA_RULES_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_stride_threat_model_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_stride_threat_model v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_STRIDE_THREAT_MODEL_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt instructs on creating a detailed threat model using the STRIDE per element methodology for a given system design document. It emphasizes understanding the system's assets, trust boundaries, and data flows to identify and prioritize potential threats. The expected output is a comprehensive table listing threats, their components, mitigation strategies, and risk assessments.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_summary_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_summary v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_SUMMARY_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes content into a structured Markdown format, focusing on brevity and clarity. It emphasizes creating a concise summary, listing main points, and identifying key takeaways. The output is organized into specific sections for easy reference.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_tags_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_tags v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_TAGS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_threat_scenarios_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_threat_scenarios v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_THREAT_SCENARIOS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt seeks to identify and prioritize potential threats to a given system or situation, using a narrative-based, simple threat modeling approach. It emphasizes distinguishing between realistic and possible threats, focusing on those worth defending against. The expected output includes a list of prioritized threat scenarios, an analysis of the threat model, recommended controls, a narrative analysis, and a concise conclusion.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_upgrade_pack_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_upgrade_pack v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_UPGRADE_PACK_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts and organizes insights on world models and task algorithms from provided content. It focuses on identifying and categorizing beliefs about the world and optimal task execution strategies. The output includes concise, actionable bullet points under relevant categories.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_video_chapters_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_video_chapters v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_VIDEO_CHAPTERS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts and organizes the most engaging topics from a transcript with corresponding timestamps. This process involves a detailed review of the transcript to identify key moments and subjects. The output is a list of topics with their timestamps in a sequential format.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_create_visualization_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Create_visualization v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            CREATE_VISUALIZATION_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Transforms complex ideas into simplified ASCII art visualizations. This approach focuses on distilling intricate concepts into visual forms that can be easily understood through ASCII art. The expected output is a detailed ASCII art representation accompanied by a concise visual explanation.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_explain_code_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Explain_code v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $CUSTOM_USER = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM, $CUSTOM_USER)
                    }}
                }}"
            "#,
            EXPLAIN_CODE_SYSTEM, EXPLAIN_CODE_USER
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes and explains code, security tool outputs, or configuration texts, tailoring the explanation to the type of input. It uses specific sections to clarify the function, implications, or settings based on the input's nature. The expected output is a detailed explanation or answer in designated sections.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_explain_docs_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Explain_docs v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXPLAIN_DOCS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt instructs on transforming input about tool usage into improved, structured documentation. It emphasizes clarity and utility, breaking down the process into specific sections for a comprehensive guide. The expected output includes an overview, usage syntax, common use cases, and key features of the tool.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_explain_project_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Explain_project v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXPLAIN_PROJECT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes project documentation into a concise, user and developer-focused summary, highlighting its purpose, problem addressed, approach, installation, usage, and examples. It simplifies complex information for easy understanding and application. The output includes a project overview, problem it addresses, approach to solving the problem, and practical steps for installation and usage.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_explain_terms_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Explain_terms v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXPLAIN_TERMS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Produces a glossary of advanced terms found in specific content, including definitions and analogies. It focuses on explaining obscure or complex terms to aid understanding. The output is a list of terms with explanations and analogies in a structured Markdown format.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_export_data_as_csv_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Export_data_as_csv v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXPORT_DATA_AS_CSV_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_algorithm_update_recommendations_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_algorithm_update_recommendations v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_ALGORITHM_UPDATE_RECOMMENDATIONS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes input to provide concise recommendations for improving processes. It focuses on extracting actionable advice from content descriptions. The output consists of a bulleted list of up to three brief suggestions.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_article_wisdom_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_article_wisdom v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $CUSTOM_USER = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM, $CUSTOM_USER)
                    }}
                }}"
            "#,
            EXTRACT_ARTICLE_WISDOM_SYSTEM, EXTRACT_ARTICLE_WISDOM_USER
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts key insights and valuable information from textual content, focusing on ideas, quotes, habits, and references. It aims to address the issue of information overload by providing a concise summary of the content's most meaningful aspects. The expected output includes summarized ideas, notable quotes, referenced materials, and habits worth adopting.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_book_ideas_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_book_ideas v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_BOOK_IDEAS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes a book's key content by extracting 50 to 100 of its most interesting ideas. The process involves a deep dive into the book's insights, prioritizing them by interest and insightfulness. The output is a concise list of bulleted ideas, limited to 20 words each.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_book_recommendations_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_book_recommendations v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_BOOK_RECOMMENDATIONS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes a book's key content by extracting 50 to 100 of its most practical recommendations, prioritizing the most impactful advice. This process involves a thorough memory search to identify actionable insights. The output is formatted as an instructive, bullet-pointed list, limited to 20 words each.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_business_ideas_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_business_ideas v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_BUSINESS_IDEAS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt outlines a process for identifying and elaborating on innovative business ideas. It focuses on extracting top business concepts from provided content and then refining the best ten by exploring adjacent possibilities. The expected output includes two sections: a list of extracted ideas and a detailed elaboration on the top ten ideas, ensuring uniqueness and differentiation.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_controversial_ideas_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_controversial_ideas v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_CONTROVERSIAL_IDEAS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_extraordinary_claims_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_extraordinary_claims v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_EXTRAORDINARY_CLAIMS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Identifies and lists extraordinary claims from conversations, focusing on those rejected by the scientific community or based on misinformation. The process involves deep analysis to pinpoint statements that defy accepted scientific truths, such as denying evolution or the moon landing. The output is a detailed list of quotes, ranging from 50 to 100, showcasing these claims.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_ideas_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_ideas v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_IDEAS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts and condenses insightful ideas from text into 15-word bullet points focusing on life's purpose and human progress. This process emphasizes capturing unique insights on specified themes. The output consists of a list of concise, thought-provoking ideas.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_insights_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_insights v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_INSIGHTS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts and condenses complex insights from text on profound topics into 15-word bullet points. This process emphasizes the extraction of nuanced, powerful ideas related to human and technological advancement. The expected output is a concise list of abstracted, insightful bullets.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_main_idea_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_main_idea v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_MAIN_IDEA_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts and highlights the most crucial or intriguing idea from any given content. This prompt emphasizes a methodical approach to identify and articulate the essence of the input. The expected output includes a concise main idea and a recommendation based on that idea.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_patterns_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_patterns v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_PATTERNS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt guides in identifying and analyzing recurring, surprising, or insightful patterns from a collection of ideas, data, or observations. It emphasizes extracting the most notable patterns based on their frequency and significance, and then documenting the process of discovery and analysis. The expected output includes a detailed summary of patterns, an explanation of their selection and significance, and actionable advice for startup builders based on these insights.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_poc_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_poc v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_POC_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes security or bug bounty reports to extract and provide proof of concept URLs for validating vulnerabilities. It specializes in identifying actionable URLs and commands from the reports, ensuring direct verification of reported vulnerabilities. The output includes the URL with a specific command to execute it, like using curl or python.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_predictions_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_predictions v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_PREDICTIONS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts and organizes predictions from content into a structured format. It focuses on identifying specific predictions, their timelines, confidence levels, and verification methods. The expected output includes a bulleted list and a detailed table of these predictions.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_questions_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_questions v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_QUESTIONS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts questions from content and analyzes their effectiveness in eliciting high-quality responses. It focuses on identifying the elements that make these questions particularly insightful. The expected output includes a list of questions, an analysis of their strengths, and recommendations for interviewers.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_recommendations_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_recommendations v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_RECOMMENDATIONS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts and condenses recommendations from content into a concise list. This process involves identifying both explicit and implicit advice within the given material. The output is a bulleted list of up to 20 brief recommendations.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_references_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_references v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_REFERENCES_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts references to various forms of cultural and educational content from provided text. This process involves identifying and listing references to art, literature, and academic papers concisely. The expected output is a bulleted list of up to 20 references, each summarized in no more than 15 words.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_song_meaning_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_song_meaning v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_SONG_MEANING_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes and interprets the meaning of songs based on extensive research and lyric examination. This process involves deep analysis of the artist's background, song context, and lyrics to deduce the song's essence. Outputs include a summary sentence, detailed meaning in bullet points, and evidence supporting the interpretation.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_sponsors_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_sponsors v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_SPONSORS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Identifies and distinguishes between official and potential sponsors from transcripts. This process involves analyzing content to separate actual sponsors from merely mentioned companies. The output lists official sponsors and potential sponsors based on their mention in the content.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_videoid_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_videoid v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_VIDEOID_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts video IDs from URLs for use in other applications. It meticulously analyzes the URL to isolate the video ID. The output is solely the video ID, with no additional information or errors included.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_wisdom_agents_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_wisdom_agents v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_WISDOM_AGENTS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("This prompt outlines a complex process for extracting insights from text content, focusing on themes like the meaning of life and technology's impact on humanity. It involves creating teams of AI agents with diverse expertise to analyze the content and produce summaries, ideas, insights, quotes, habits, facts, references, and recommendations. The expected output includes structured sections filled with concise, insightful entries derived from the input material.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_wisdom_dm_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_wisdom_dm v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_WISDOM_DM_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts and synthesizes valuable content from input text, focusing on insights related to life's purpose and human advancement. It employs a structured approach to distill surprising ideas, insights, quotes, habits, facts, and recommendations from the content. The output includes summaries, ideas, insights, and other categorized information for deep understanding and practical application.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_wisdom_nometa_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_wisdom_nometa v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_WISDOM_NOMETA_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("This prompt guides the extraction and organization of insightful content from text, focusing on life's purpose, human flourishing, and technology's impact. It emphasizes identifying and summarizing surprising ideas, refined insights, practical habits, notable quotes, valid facts, and useful recommendations related to these themes. The expected output includes structured sections for summaries, ideas, insights, quotes, habits, facts, recommendations, and references, each with specific content and formatting requirements.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_extract_wisdom_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Extract_wisdom v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            EXTRACT_WISDOM_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts key insights, ideas, quotes, habits, and references from textual content to address the issue of information overload and the challenge of retaining knowledge. It uniquely filters and condenses valuable information from various texts, making it easier for users to decide if the content warrants a deeper review or to use as a note-taking alternative. The output includes summarized ideas, notable quotes, relevant habits, and useful references, all aimed at enhancing understanding and retention.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_find_hidden_message_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Find_hidden_message v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            FIND_HIDDEN_MESSAGE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes political messages to reveal overt and hidden intentions. It employs knowledge of politics, propaganda, and psychology to dissect content, focusing on recent political debates. The output includes overt messages, hidden cynical messages, supporting arguments, desired audience actions, and analyses from cynical to favorable.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_find_logical_fallacies_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Find_logical_fallacies v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            FIND_LOGICAL_FALLACIES_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Identifies and categorizes various fallacies in arguments or texts. This prompt focuses on recognizing invalid or faulty reasoning across a wide range of fallacies, from formal to informal types. The expected output is a list of identified fallacies with brief explanations.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_get_wow_per_minute_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Get_wow_per_minute v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            GET_WOW_PER_MINUTE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Evaluates the density of wow-factor in content by analyzing its surprise, novelty, insight, value, and wisdom. This process involves a detailed and varied consumption of the content to assess its potential to engage and enrich viewers. The expected output is a JSON report detailing scores and explanations for each wow-factor component and overall wow-factor per minute.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_get_youtube_rss_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Get_youtube_rss v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            GET_YOUTUBE_RSS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates RSS URLs for YouTube channels based on given channel IDs or URLs. It extracts the channel ID from the input and constructs the corresponding RSS URL. The output is solely the RSS URL.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_improve_academic_writing_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Improve_academic_writing v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            IMPROVE_ACADEMIC_WRITING_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("This prompt aims to enhance the quality of text for academic purposes. It focuses on refining grammatical errors, improving clarity and coherence, and adopting an academic tone while ensuring ease of understanding. The expected output is a professionally refined text with a list of applied corrections.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_improve_prompt_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Improve_prompt v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            IMPROVE_PROMPT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("This service enhances LLM/AI prompts by applying expert prompt writing techniques to achieve better results. It leverages strategies like clear instructions, persona adoption, and reference text provision to refine prompts. The output is an improved version of the original prompt, optimized for clarity and effectiveness.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_improve_report_finding_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Improve_report_finding v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $CUSTOM_USER = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM, $CUSTOM_USER)
                    }}
                }}"
            "#,
            IMPROVE_REPORT_FINDING_SYSTEM, IMPROVE_REPORT_FINDING_USER
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt instructs the creation of an improved security finding report from a penetration test, detailing the finding, risk, recommendations, references, a concise summary, and insightful quotes, all formatted in markdown without using markdown syntax or special formatting. It emphasizes a detailed, insightful approach to presenting cybersecurity issues and solutions. The output should be comprehensive, covering various sections including title, description, risk, recommendations, references, and quotes, aiming for clarity and depth in reporting.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_improve_writing_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Improve_writing v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            IMPROVE_WRITING_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("This prompt aims to refine input text for enhanced clarity, coherence, grammar, and style. It involves analyzing the text for errors and inconsistencies, then applying corrections while preserving the original meaning. The expected output is a grammatically correct and stylistically improved version of the text.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_label_and_rate_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Label_and_rate v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            LABEL_AND_RATE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Evaluates and categorizes content based on its relevance to specific human-centric themes, then assigns a tiered rating and a numerical quality score. It uses a predefined set of labels for categorization and assesses content based on idea quantity and thematic alignment. The expected output is a structured JSON object detailing the content summary, labels, rating, and quality score with explanations.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_official_pattern_template_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Official_pattern_template v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            OFFICIAL_PATTERN_TEMPLATE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt outlines a complex process for diagnosing and addressing psychological issues based on a person's background and behaviors. It involves deep analysis of the individual's history, identifying potential mental health issues, and suggesting corrective actions. The expected output includes summaries of past events, possible psychological issues, their impact on behavior, and recommendations for improvement.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_provide_guidance_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Provide_guidance v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            PROVIDE_GUIDANCE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Provides comprehensive psychological advice tailored to the individual's specific question and context. This approach delves into the person's past, traumas, and life goals to offer targeted feedback and recommendations. The expected output includes a concise analysis, detailed scientific rationale, actionable recommendations, Esther Perel's perspective, self-reflection prompts, possible clinical diagnoses, and a summary, all aimed at fostering self-awareness and positive change.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_rate_ai_response_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Rate_ai_response v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            RATE_AI_RESPONSE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Evaluates the quality of AI responses against the benchmark of human experts, assigning a letter grade and score. It involves deep analysis of both the instructions given to the AI and its output, comparing these to the potential performance of the world's best human expert. The process culminates in a detailed justification for the assigned grade, highlighting specific strengths and weaknesses of the AI's response.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_rate_ai_result_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Rate_ai_result v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            RATE_AI_RESULT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Evaluates the quality of AI-generated content based on construction, quality, and spirit. The process involves analyzing AI outputs against criteria set by experts and a high-IQ AI panel. The expected output is a final score out of 100, with deductions detailed for each category.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_rate_content_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Rate_content v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $CUSTOM_USER = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM, $CUSTOM_USER)
                    }}
                }}"
            "#,
            RATE_CONTENT_SYSTEM, RATE_CONTENT_USER
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt outlines a process for evaluating content by labeling it with relevant single-word descriptors, rating its quality based on idea quantity and thematic alignment, and scoring it on a scale from 1 to 100. It emphasizes the importance of matching content with specific themes related to human meaning and the future of AI, among others. The expected output includes a list of labels, a tiered rating with an explanation, and an overall quality score with justification.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_rate_value_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Rate_value v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            RATE_VALUE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("This prompt seeks to acknowledge the collaborative effort behind its creation, inspired by notable figures in information theory and viral content creation. It highlights the fusion of theoretical foundations and modern digital strategies. The output is an attribution of credit.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_raw_query_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Raw_query v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            RAW_QUERY_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt instructs the AI to produce the best possible output by thoroughly analyzing and understanding the input. It emphasizes deep contemplation of the input's meaning and the sender's intentions. The expected output is an optimal response tailored to the inferred desires of the input provider.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_recommend_artists_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Recommend_artists v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            RECOMMEND_ARTISTS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Recommends a personalized festival schedule featuring artists similar to the user's preferences in EDM genres and artists. The recommendation process involves analyzing the user's favorite styles and artists, then selecting similar artists and explaining the choices. The output is a detailed schedule organized by day, set time, stage, and artist, optimized for the user's enjoyment.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_show_fabric_options_markmap_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Show_fabric_options_markmap v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SHOW_FABRIC_OPTIONS_MARKMAP_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Create a visual representation of the functionalities provided by the Fabric project, focusing on augmenting human capabilities with AI. The approach involves breaking down the project's capabilities into categories like summarization, analysis, and more, with specific patterns branching from these categories. The expected output is comprehensive Markmap code detailing this functionality map.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_suggest_pattern_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Suggest_pattern v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $CUSTOM_USER = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM, $CUSTOM_USER)
                    }}
                }}"
            "#,
            SUGGEST_PATTERN_SYSTEM.replace('"', "\\\""),
            SUGGEST_PATTERN_USER.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_debate_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_debate v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_DEBATE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes debates to identify and summarize the primary disagreements, arguments, and evidence that could change participants' minds. It breaks down complex discussions into concise summaries and evaluates argument strength, predicting outcomes. The output includes structured summaries and analyses of each party's position and evidence.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_git_changes_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_git_changes v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_GIT_CHANGES_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes major changes and upgrades in a GitHub project over the past week. It involves identifying key updates, then crafting a concise, enthusiastic summary and detailed bullet points highlighting these changes. The output includes a 20-word introduction and excitedly written update bullets.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_git_diff_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_git_diff v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_GIT_DIFF_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Analyzes Git diffs to summarize major changes and upgrades. It emphasizes creating concise bullet points for feature changes and updates, tailored to the extent of modifications. The expected output includes a 100-character intro sentence using conventional commits format.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_lecture_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_lecture v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_LECTURE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_legislation_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_legislation v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_LEGISLATION_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_micro_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_micro v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_MICRO_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes content into a structured Markdown format. This prompt focuses on concise, bullet-pointed summaries and takeaways. The output includes a one-sentence summary and lists of main points and takeaways.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_newsletter_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_newsletter v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_NEWSLETTER_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Extracts and organizes key content from newsletters, focusing on the most meaningful, interesting, and useful information. It uniquely parses the entire newsletter to provide concise summaries, lists of content, opinions, tools, companies, and follow-up actions. The output includes sections for a brief summary, detailed content points, author opinions, mentioned tools and companies, and recommended follow-ups in a structured Markdown format.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_paper_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_paper v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_PAPER_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes academic papers by extracting key sections such as title, authors, main goals, and more from the provided text. It employs a structured approach to highlight the paper's core aspects including technical methodology, distinctive features, and experimental outcomes. The output is a detailed summary covering various dimensions of the research.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_prompt_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_prompt v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_PROMPT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_pull_requests_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_pull_requests v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_PULL_REQUESTS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes pull requests for a coding project, focusing on the types of changes made. It involves creating a summary and a detailed list of main PRs, rewritten for clarity. The output includes a concise overview and specific examples of pull requests.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_rpg_session_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_rpg_session v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_RPG_SESSION_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("This prompt outlines the process for summarizing in-person role-playing game sessions, focusing on key events, combat details, character development, and worldbuilding. It emphasizes capturing the essence of the session in a structured format, including summaries, lists, and descriptions to encapsulate the narrative and gameplay dynamics. The expected output includes a comprehensive overview of the session's storyline, character interactions, and significant moments, tailored for both players and observers.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Summarizes content into a structured Markdown format, focusing on brevity and clarity. It extracts and lists the most crucial points and takeaways. The output includes a one-sentence summary, main points, and key takeaways, adhering to specified word limits.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_to_flashcards_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow To_flashcards v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            TO_FLASHCARDS_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Creates Anki cards from texts following specific principles to ensure simplicity, optimized wording, and no reliance on external context. This approach aims to enhance learning efficiency and comprehension without requiring prior knowledge of the text. The expected output is a set of questions and answers formatted as a CSV table.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_tweet_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Tweet v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            TWEET_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Guides users on crafting engaging tweets with emojis, focusing on Twitter's basics and content creation strategies. It emphasizes understanding Twitter, identifying the target audience, and using emojis effectively. The expected output is a comprehensive guide for creating appealing tweets with emojis.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_write_essay_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Write_essay v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            WRITE_ESSAY_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The task is to write an essay in the style of Paul Graham, focusing on the essence and approach of writing concise, clear, and illuminating essays on any given topic.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_write_hackerone_report_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Write_hackerone_report v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            WRITE_HACKERONE_REPORT_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_write_micro_essay_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Write_micro_essay v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            WRITE_MICRO_ESSAY_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The task is to write an essay in the style of Paul Graham, focusing on the essence of simplicity in conveying complex ideas.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_write_pull_request_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Write_pull_request v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            WRITE_PULL_REQUEST_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt instructs on drafting a detailed pull request (PR) description based on the output of a `git diff` command, focusing on identifying and explaining code changes. It emphasizes analyzing changes, understanding their purpose, and detailing their impact on the project. The expected output is a structured PR description in markdown, covering a summary of changes, reasons, impacts, and testing plans in clear language.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_write_semgrep_rule_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Write_semgrep_rule v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            WRITE_SEMGREP_RULE_SYSTEM.replace('"', "\\\"")
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("The prompt requests the creation of a Semgrep rule to detect a specific vulnerability pattern in code, based on provided context and examples. It emphasizes the importance of crafting a rule that is general enough to catch any instance of the described vulnerability, rather than being overly specific to the given examples. The expected output is a well-structured Semgrep rule that aligns with the syntax and guidelines detailed in the context, capable of identifying the vulnerability across different scenarios.".to_string());

        WorkflowTool::new(workflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_serialize_workflow_tool() {
        let raw_workflow = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            }
        "#;

        let workflow = parse_workflow(raw_workflow).expect("Failed to parse workflow");
        let workflow_tool = WorkflowTool::new(workflow);

        let serialized = serde_json::to_string(&workflow_tool).expect("Failed to serialize WorkflowTool");
        println!("{}", serialized);

        // Optionally, you can add assertions to check the serialized output
        assert!(serialized.contains("ExtensiveSummary"));
    }

    #[test]
    fn test_get_db_key() {
        let raw_workflow = r#"
            workflow ExtensiveSummary v0.1 {
                step Initialize {
                    $PROMPT = "Summarize this: "
                    $EMBEDDINGS = call process_embeddings_in_job_scope()
                }
                step Summarize {
                    $RESULT = call multi_inference($PROMPT, $EMBEDDINGS)
                }
            }
        "#;

        let workflow = parse_workflow(raw_workflow).expect("Failed to parse workflow");
        let workflow_tool = WorkflowTool::new(workflow);

        assert_eq!(workflow_tool.get_db_key(), "ExtensiveSummary:::v0.1");
    }
}
