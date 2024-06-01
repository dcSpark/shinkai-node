#[cfg(test)]
mod tests {
    use identifier_parser::Rule;
    use pest::Parser;

    mod identifier_parser {
        use pest_derive::Parser;

        #[derive(Parser)]
        #[grammar_inline = r#"
        identifier = _{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
        "#]
        pub struct IdentifierParser;
    }

    mod input_parser {
        use pest_derive::Parser;

        #[derive(Parser)]
        #[grammar_inline = r#"
        input      =  { "input" ~ identifier ~ ":" ~ identifier }
        identifier = { (ASCII_ALPHA | "_")+ ~ (ASCII_ALPHANUMERIC | "_")* }
        WHITESPACE = _{ " " | "\t" }
        "#]
        pub struct InputParser;
    }

    mod output_parser {
        use pest_derive::Parser;

        #[derive(Parser)]
        #[grammar_inline = r#"
        output = { "output" ~ identifier ~ ":" ~ identifier }
        identifier = { (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
        WHITESPACE = _{ " " | "\t" }
        "#]
        pub struct OutputParser;
    }

    mod condition_parser {
        use pest_derive::Parser;

        #[derive(Parser)]
        #[grammar_inline = r#"
        condition = { "if" ~ expression }
        expression = { identifier ~ comparison_operator ~ value }
        comparison_operator = { "==" | "!=" | ">" | "<" | ">=" | "<=" }
        value = { string | number | boolean | identifier }
        identifier = _{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
        string = _{ "\"" ~ (!"\"" ~ ANY)* ~ "\"" }
        number = _{ ASCII_DIGIT+ }
        boolean = { "true" | "false" }
        WHITESPACE = _{ " " | "\t" }
        "#]
        pub struct ConditionParser;
    }

    mod action_parser {
        use pest_derive::Parser;

        #[derive(Parser)]
        #[grammar_inline = r#"
        action = { command ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" | external_fn_call }
        command = { identifier }
        param = { string | number | boolean | identifier }
        external_fn_call = { "call" ~ identifier ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" }
        identifier = _{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
        string = _{ "\"" ~ (!"\"" ~ ANY)* ~ "\"" }
        number = _{ ASCII_DIGIT+ }
        boolean = { "true" | "false" }
        WHITESPACE = _{ " " | "\t" }
        "#]
        pub struct ActionParser;
    }

    mod step_parser {
        use pest_derive::Parser;

        #[derive(Parser)]
        #[grammar_inline = r#"
        step                =  { "step" ~ identifier ~ "{" ~ step_body ~ "}" }
        step_body           =  { condition ~ action ~ "}" | action }
        condition           =  { "if" ~ expression ~ "{" }
        action              =  { external_fn_call | command ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" }
        command             =  { identifier }
        param               =  { string | number | boolean | identifier }
        external_fn_call    =  { "call" ~ identifier ~ "(" ~ (param ~ ("," ~ param)*)? ~ ")" }
        expression          =  { identifier ~ comparison_operator ~ value }
        comparison_operator =  { "==" | "!=" | ">" | "<" | ">=" | "<=" }
        value               =  { string | number | boolean | identifier }
        identifier          = _{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
        string              = _{ "\"" ~ (!"\"" ~ ANY)* ~ "\"" }
        number              = _{ ASCII_DIGIT+ }
        boolean             =  { "true" | "false" }
        WHITESPACE          = _{ " " | "\t" | "\n" | "\r" }
        "#]
        pub struct StepParser;
    }

    #[test]
    fn test_identifier() {
        let valid_identifiers = vec!["agent1", "my_agent", "_agent"];
        for id in valid_identifiers {
            assert!(identifier_parser::IdentifierParser::parse(Rule::identifier, id).is_ok());
        }
    }

    #[test]
    fn test_input() {
        use input_parser::InputParser;
        use input_parser::Rule;

        let input_str = "input topic: identifier";
        let parse_result = InputParser::parse(Rule::input, input_str);
        assert!(parse_result.is_ok());
    }

    #[test]
    fn test_output() {
        use output_parser::OutputParser;
        use output_parser::Rule;

        let output_str = "output perspectives: List<String>";
        let parse_result = OutputParser::parse(Rule::output, output_str);
        assert!(parse_result.is_ok());
    }

    #[test]
    fn test_condition() {
        use condition_parser::ConditionParser;
        use condition_parser::Rule;

        let condition_str = "if perspectives != \"\"";
        let parse_result = ConditionParser::parse(Rule::condition, condition_str);
        assert!(parse_result.is_ok());
    }

    #[test]
    fn test_action() {
        use action_parser::ActionParser;
        use action_parser::Rule;

        let action_str = "generate_questions(perspectives)";
        let parse_result = ActionParser::parse(Rule::action, action_str);
        assert!(parse_result.is_ok());
    }

    #[test]
    fn test_step() {
        use step_parser::Rule;
        use step_parser::StepParser;

        let step_str = r#"step GenerateQuestions { 
            if perspectives != "" { 
                call generate_questions(perspectives) 
            } 
        }"#;
        let parse_result = StepParser::parse(Rule::step, step_str);
        assert!(parse_result.is_ok());
    }

    #[test]
    fn test_action_command() {
        use action_parser::ActionParser;
        use action_parser::Rule;

        let action_str = "update_data(\"new_data\", 123, true)";
        let parse_result = ActionParser::parse(Rule::action, action_str);
        assert!(parse_result.is_ok());
    }
}
