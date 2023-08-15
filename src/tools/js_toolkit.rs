use crate::tools::auth::ToolAuth;
use crate::tools::error::ToolError;
use crate::tools::js_tools::JSTool;
use serde_json::Value as JsonValue;

pub struct JSToolkit {
    pub name: String,
    pub tools: Vec<JSTool>,
    pub auth: Option<ToolAuth>,
}

impl JSToolkit {
    pub fn from_toolkit_json(json: &str) -> Result<Self, ToolError> {
        let parsed_json: JsonValue = serde_json::from_str(json)?;

        let name = parsed_json["name"]
            .as_str()
            .ok_or(ToolError::ParseError("name".to_string()))?;

        let tools_json = parsed_json["tools"]
            .as_array()
            .ok_or(ToolError::ParseError("tools".to_string()))?;
        let mut tools = Vec::new();
        for tool_json in tools_json {
            let tool = JSTool::from_toolkit_json(name, tool_json)?;
            tools.push(tool);
        }

        Ok(Self {
            name: name.to_string(),
            tools,
            auth: None, // Assuming no auth data in the JSON
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_toolkit_json() -> String {
        r#"{"name":"Google Calendar Toolkit","description":"A set of tools for reading/writing to the user's Google calendar.","tools":[{"name":"Create Google Calendar Event","description":"Creates a new event on the specificied calendar.","input":[{"name":"calendar_id","type":"STRING","description":"ID of the calendar to create an event on. Primary calendar used if not specified","isOptional":true,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)?"},{"name":"text","type":"STRING","description":"The text that will be attached to the event which describes the event","isOptional":false,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)"},{"name":"send_updates","type":"ENUM","description":"Guests who should receive notifications about the creation of the new event.","isOptional":true,"wrapperType":"none","enum":["all","externalOnly","none"],"ebnf":"(\"all\" | \"externalOnly\" | \"none\")?"}],"output":[{"name":"response","type":"STRING","description":"Network Response","isOptional":false,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)"}],"inputEBNF":"{ \"calendar_id\" : calendar_id,\"text\" : text,\"send_updates\" : send_updates }\n          calendar_id ::= ([a-zA-Z0-9_]+)?\ntext ::= ([a-zA-Z0-9_]+)\nsend_updates ::= (\"all\" | \"externalOnly\" | \"none\")?}"}],"oauth":{"description":"","displayName":"Authentication","authUrl":"https://accounts.google.com/o/oauth2/auth","tokenUrl":"https://oauth2.googleapis.com/token","required":true,"pkce":true,"scope":["https://www.googleapis.com/auth/calendar.events","https://www.googleapis.com/auth/calendar.readonly"],"cloudOAuth":"activepieces"},"executionSetup":[{"name":"x-shinkai-oauth","type":"STRING","description":"OAuth Token.","isOptional":false,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)"}]}"#.to_string()
    }

    #[test]
    fn test_js_toolkit_json_parsing() {
        let toolkit = JSToolkit::from_toolkit_json(&default_toolkit_json()).unwrap();
        assert_eq!(toolkit.name, "Google Calendar Toolkit");
    }

    #[test]
    fn test_js_toolkit_ebnf_output() {
        let toolkit = JSToolkit::from_toolkit_json(&default_toolkit_json()).unwrap();
        assert_eq!(
            toolkit.tools[0].ebnf_inputs(false).replace("\n", ""),
            r#"{"calendar_id": calendar_id, "text": text, "send_updates": send_updates, }calendar_id :== ([a-zA-Z0-9_]+)?text :== ([a-zA-Z0-9_]+)send_updates :== ("all" | "externalOnly" | "none")?"#
        );

        assert_eq!(
            toolkit.tools[0].ebnf_inputs(true).replace("\n", ""),
            r#"{"calendar_id": calendar_id, "text": text, "send_updates": send_updates, }\ncalendar_id :== ([a-zA-Z0-9_]+)? (* ID of the calendar to create an event on. Primary calendar used if not specified *)\ntext :== ([a-zA-Z0-9_]+) (* The text that will be attached to the event which describes the event *)\nsend_updates :== ("all" | "externalOnly" | "none")? (* Guests who should receive notifications about the creation of the new event. *)\n"#.replace("\n", "").replace("\\n", "")
        );
    }
}
