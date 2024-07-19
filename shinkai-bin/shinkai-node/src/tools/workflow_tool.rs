use shinkai_dsl::{dsl_schemas::Workflow, parser::parse_workflow};
use shinkai_vector_resources::embeddings::Embedding;

use super::{
    argument::ToolArgument,
    workflow_static_texts::{
        AGILITY_STORY_SYSTEM, AI_SYSTEM, ANALYZE_ANSWERS_SYSTEM, ANALYZE_CLAIMS_SYSTEM, ANALYZE_DEBATE_SYSTEM, ANALYZE_INCIDENT_SYSTEM, ANALYZE_LOGS_SYSTEM, ANALYZE_MALWARE_SYSTEM, ANALYZE_PAPER_SYSTEM, ANALYZE_PATENT_SYSTEM, ANALYZE_PERSONALITY_SYSTEM, ANALYZE_PRESENTATION_SYSTEM, ANALYZE_PROSE_JSON_SYSTEM, ANALYZE_PROSE_PINKER_SYSTEM, ANALYZE_PROSE_SYSTEM, ANALYZE_SPIRITUAL_TEXT_SYSTEM, ANALYZE_TECH_IMPACT_SYSTEM, ANALYZE_THREAT_REPORT_SYSTEM, ANALYZE_THREAT_REPORT_TRENDS_SYSTEM, ANALYZE_THREAT_REPORT_TRENDS_USER, ANALYZE_THREAT_REPORT_USER, ANSWER_INTERVIEW_QUESTION_SYSTEM, ASK_SECURE_BY_DESIGN_QUESTIONS_SYSTEM, CAPTURE_THINKERS_WORK_SYSTEM, CHECK_AGREEMENT_SYSTEM, CLEAN_TEXT_SYSTEM, CODING_MASTER_SYSTEM, COMPARE_AND_CONTRAST_SYSTEM, CREATE_5_SENTENCE_SUMMARY_SYSTEM, CREATE_ACADEMIC_PAPER_SYSTEM, CREATE_AI_JOBS_ANALYSIS_SYSTEM, CREATE_APHORISMS_SYSTEM, CREATE_ART_PROMPT_SYSTEM, CREATE_BETTER_FRAME_SYSTEM, CREATE_CODING_PROJECT_SYSTEM, CREATE_COMMAND_SYSTEM, CREATE_CYBER_SUMMARY_SYSTEM, CREATE_GIT_DIFF_COMMIT_SYSTEM, CREATE_GRAPH_FROM_INPUT_SYSTEM, CREATE_HORMOZI_OFFER_SYSTEM, CREATE_IDEA_COMPASS_SYSTEM, CREATE_INVESTIGATION_VISUALIZATION_SYSTEM, CREATE_KEYNOTE_SYSTEM, CREATE_LOGO_SYSTEM, CREATE_MARKMAP_VISUALIZATION_SYSTEM, CREATE_MERMAID_VISUALIZATION_SYSTEM, CREATE_MICRO_SUMMARY_SYSTEM, CREATE_NETWORK_THREAT_LANDSCAPE_SYSTEM, CREATE_NETWORK_THREAT_LANDSCAPE_USER, CREATE_NPC_SYSTEM, CREATE_PATTERN_SYSTEM, CREATE_QUIZ_SYSTEM, CREATE_READING_PLAN_SYSTEM, CREATE_REPORT_FINDING_SYSTEM, CREATE_REPORT_FINDING_USER, CREATE_SECURITY_UPDATE_SYSTEM, CREATE_SHOW_INTRO_SYSTEM, CREATE_SIGMA_RULES_SYSTEM, CREATE_STRIDE_THREAT_MODEL_SYSTEM, CREATE_SUMMARY_SYSTEM, CREATE_TAGS_SYSTEM, CREATE_THREAT_SCENARIOS_SYSTEM, CREATE_UPGRADE_PACK_SYSTEM, CREATE_VIDEO_CHAPTERS_SYSTEM, CREATE_VISUALIZATION_SYSTEM, EXPLAIN_CODE_SYSTEM, EXPLAIN_CODE_USER, EXPLAIN_DOCS_SYSTEM, EXPLAIN_PROJECT_SYSTEM, EXPLAIN_TERMS_SYSTEM, EXPORT_DATA_AS_CSV_SYSTEM, EXTRACT_ALGORITHM_UPDATE_RECOMMENDATIONS_SYSTEM, EXTRACT_ARTICLE_WISDOM_SYSTEM, EXTRACT_ARTICLE_WISDOM_USER, EXTRACT_BOOK_IDEAS_SYSTEM, EXTRACT_BOOK_RECOMMENDATIONS_SYSTEM, EXTRACT_BUSINESS_IDEAS_SYSTEM, EXTRACT_CONTROVERSIAL_IDEAS_SYSTEM, EXTRACT_EXTRAORDINARY_CLAIMS_SYSTEM, EXTRACT_IDEAS_SYSTEM, EXTRACT_INSIGHTS_SYSTEM, EXTRACT_MAIN_IDEA_SYSTEM, EXTRACT_PATTERNS_SYSTEM, EXTRACT_POC_SYSTEM, EXTRACT_PREDICTIONS_SYSTEM, EXTRACT_QUESTIONS_SYSTEM, EXTRACT_RECOMMENDATIONS_SYSTEM, EXTRACT_REFERENCES_SYSTEM, EXTRACT_SONG_MEANING_SYSTEM, EXTRACT_SPONSORS_SYSTEM, EXTRACT_VIDEOID_SYSTEM, EXTRACT_WISDOM_AGENTS_SYSTEM, EXTRACT_WISDOM_DM_SYSTEM, EXTRACT_WISDOM_NOMETA_SYSTEM, EXTRACT_WISDOM_SYSTEM, FIND_HIDDEN_MESSAGE_SYSTEM, FIND_LOGICAL_FALLACIES_SYSTEM, GENERATE_QUIZ_SYSTEM, GET_WOW_PER_MINUTE_SYSTEM, GET_YOUTUBE_RSS_SYSTEM, IMPROVE_ACADEMIC_WRITING_SYSTEM, IMPROVE_PROMPT_SYSTEM, IMPROVE_REPORT_FINDING_SYSTEM, IMPROVE_REPORT_FINDING_USER, IMPROVE_WRITING_SYSTEM, LABEL_AND_RATE_SYSTEM, OFFICIAL_PATTERN_TEMPLATE_SYSTEM, PROVIDE_GUIDANCE_SYSTEM, RATE_AI_RESPONSE_SYSTEM, RATE_AI_RESULT_SYSTEM, RATE_CONTENT_SYSTEM, RATE_CONTENT_USER, RATE_VALUE_SYSTEM, RAW_QUERY_SYSTEM, RECOMMEND_ARTISTS_SYSTEM, SHOW_FABRIC_OPTIONS_MARKMAP_SYSTEM, SUGGEST_PATTERN_SYSTEM, SUGGEST_PATTERN_USER, SUMMARIZE_DEBATE_SYSTEM, SUMMARIZE_GIT_CHANGES_SYSTEM, SUMMARIZE_GIT_DIFF_SYSTEM, SUMMARIZE_LECTURE_SYSTEM, SUMMARIZE_LEGISLATION_SYSTEM, SUMMARIZE_MICRO_SYSTEM, SUMMARIZE_NEWSLETTER_SYSTEM, SUMMARIZE_PAPER_SYSTEM, SUMMARIZE_PROMPT_SYSTEM, SUMMARIZE_PULL_REQUESTS_SYSTEM, SUMMARIZE_RPG_SESSION_SYSTEM, SUMMARIZE_SYSTEM, TO_FLASHCARDS_SYSTEM, TWEET_SYSTEM, WRITE_ESSAY_SYSTEM, WRITE_HACKERONE_REPORT_SYSTEM, WRITE_MICRO_ESSAY_SYSTEM, WRITE_PULL_REQUEST_SYSTEM, WRITE_SEMGREP_RULE_SYSTEM
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
        vec![
            Self::get_extensive_summary_workflow(),
            Self::get_hyde_inference_workflow(),
            Self::get_generate_quiz_workflow(),
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
            GENERATE_QUIZ_SYSTEM
        );

        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates quiz questions based on the provided learning objectives.".to_string());

        WorkflowTool::new(workflow)
    }

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
            AGILITY_STORY_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            AI_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_ANSWERS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_CLAIMS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_DEBATE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_INCIDENT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_LOGS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_MALWARE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_PAPER_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_PATENT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_PERSONALITY_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_PRESENTATION_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_PROSE_JSON_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_PROSE_PINKER_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_PROSE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_SPIRITUAL_TEXT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANALYZE_TECH_IMPACT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ANSWER_INTERVIEW_QUESTION_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            ASK_SECURE_BY_DESIGN_QUESTIONS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CAPTURE_THINKERS_WORK_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CHECK_AGREEMENT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CLEAN_TEXT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CODING_MASTER_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            COMPARE_AND_CONTRAST_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_5_SENTENCE_SUMMARY_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_ACADEMIC_PAPER_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_AI_JOBS_ANALYSIS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_APHORISMS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_ART_PROMPT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_BETTER_FRAME_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_CODING_PROJECT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_COMMAND_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_CYBER_SUMMARY_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_GIT_DIFF_COMMIT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_GRAPH_FROM_INPUT_SYSTEM
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
            CREATE_HORMOZI_OFFER_SYSTEM
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
            CREATE_IDEA_COMPASS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_INVESTIGATION_VISUALIZATION_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_KEYNOTE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_LOGO_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_MARKMAP_VISUALIZATION_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_MERMAID_VISUALIZATION_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_MICRO_SUMMARY_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_NPC_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_PATTERN_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_QUIZ_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_READING_PLAN_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_SECURITY_UPDATE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_SHOW_INTRO_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_SIGMA_RULES_SYSTEM
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
            CREATE_STRIDE_THREAT_MODEL_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_SUMMARY_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_TAGS_SYSTEM
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
            CREATE_THREAT_SCENARIOS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_UPGRADE_PACK_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_VIDEO_CHAPTERS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            CREATE_VISUALIZATION_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXPLAIN_DOCS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXPLAIN_PROJECT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXPLAIN_TERMS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXPORT_DATA_AS_CSV_SYSTEM
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
            EXTRACT_ALGORITHM_UPDATE_RECOMMENDATIONS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_BOOK_IDEAS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_BOOK_RECOMMENDATIONS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_BUSINESS_IDEAS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_CONTROVERSIAL_IDEAS_SYSTEM
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
            EXTRACT_EXTRAORDINARY_CLAIMS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_IDEAS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_INSIGHTS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_MAIN_IDEA_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_PATTERNS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_POC_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_PREDICTIONS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_QUESTIONS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_RECOMMENDATIONS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_REFERENCES_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_SONG_MEANING_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_SPONSORS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_VIDEOID_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_WISDOM_AGENTS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_WISDOM_DM_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_WISDOM_NOMETA_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            EXTRACT_WISDOM_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            FIND_HIDDEN_MESSAGE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            FIND_LOGICAL_FALLACIES_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            GET_WOW_PER_MINUTE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            GET_YOUTUBE_RSS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            IMPROVE_ACADEMIC_WRITING_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            IMPROVE_PROMPT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            IMPROVE_WRITING_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            LABEL_AND_RATE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            OFFICIAL_PATTERN_TEMPLATE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            PROVIDE_GUIDANCE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            RATE_AI_RESPONSE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            RATE_AI_RESULT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            RATE_VALUE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            RAW_QUERY_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            RECOMMEND_ARTISTS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SHOW_FABRIC_OPTIONS_MARKMAP_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SUGGEST_PATTERN_SYSTEM, SUGGEST_PATTERN_USER
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
            SUMMARIZE_DEBATE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SUMMARIZE_GIT_CHANGES_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SUMMARIZE_GIT_DIFF_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SUMMARIZE_LECTURE_SYSTEM
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
            SUMMARIZE_LEGISLATION_SYSTEM
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
            SUMMARIZE_MICRO_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SUMMARIZE_NEWSLETTER_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SUMMARIZE_PAPER_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SUMMARIZE_PROMPT_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_summarize_pull_requests_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Summarize_pull-requests v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            SUMMARIZE_PULL_REQUESTS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SUMMARIZE_RPG_SESSION_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            SUMMARIZE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            TO_FLASHCARDS_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            TWEET_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            WRITE_ESSAY_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            WRITE_HACKERONE_REPORT_SYSTEM
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
            WRITE_MICRO_ESSAY_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

        WorkflowTool::new(workflow)
    }

    fn get_write_pull_request_workflow() -> Self {
        let raw_workflow = format!(
            r#"
                workflow Write_pull-request v0.1 {{
                    step Main {{
                        $SYSTEM = "{}"
                        $RESULT = call opinionated_inference($INPUT, $SYSTEM)
                    }}
                }}"
            "#,
            WRITE_PULL_REQUEST_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
            WRITE_SEMGREP_RULE_SYSTEM
        );
        let mut workflow = parse_workflow(&raw_workflow).expect("Failed to parse workflow");
        workflow.description = Some("Generates workflow based on the provided system.md.".to_string());

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
