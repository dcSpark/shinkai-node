use lazy_static::lazy_static;

lazy_static! {
    static ref task_bootstrap_prompt: String = String::from(
        r#"
    You are an assistant running in a system who only has access to a series of tools and your own knowledge. The user has asked you:

    `What is the weather like today in New York?`

    
    If it is a task not pertaining to recent/current knowledge and you can respond respond directly without any external help, respond using the following EBNF and absolutely nothing else:

    `"{" "answer" ":" string "}"`

    If you do not have the ability to respond correctly yourself, it is your goal is to find the final tool that will provide you with the capabilities you need. 
    Search to find tools which you can use, respond using the following EBNF and absolutely nothing else:

    "{" ("tool-search" ":" string) "}"

    Only respond with an answer if you are not using any tools. Make sure the response matches the EBNF and includes absolutely nothing else. 

    ```json
    "#
    );
    static ref tool_selection_prompt: String = String::from(
        r#"

    You are an assistant running in a system who only has access to a series of tools and your own knowledge. The user has asked the system:

    `What is the weather like today in New York?`

    Here are up to 10 of the most relevant tools available:
    1. Name: Weather Fetch - Description: Requests weather via an API given a city name.
    2. Name: Country Population - Description: Provides population numbers given a country name.
    3. Name: HTTP GET - Description: Issues an http get request to a specified URL. Note: Only fetch URLs from user's input or from output of other tools.

    It is your goal to select the final tool that will enable the system to accomplish the user's task. The system may end up needing to chain multiple tools to acquire all needed info/data, but the goal right now is to find the final tool.
    Select the name of the tool from the list above that fulfill this, respond using the following EBNF and absolutely nothing else:

    "{" ("tool" ":" string) "}"

    If none of the tools match explain what the issue is by responding using the following EBNF and absolutely nothing else:

    "{" ("error" ":" string) "}"


    ```json



        "#
    );
    static ref tool_ebnf_prompt: String = String::from(
        r#"

    You are an assistant running in a system who only has access to a series of tools and your own knowledge. The user has asked the system:

    `What is the weather like today in New York?`

    The system has selected the following tool to be used:

    Name: Weather Fetch
    Description: Requests weather via an API given a city name.
    Tool Input EBNF: ...
    Tool Output EBNF: ...

    Your goal is to decide whether you have all of the information you need to fill out the Tool Input EBNF.

    If all of the data/information to use the tool is available, respond using the following EBNF and absolutely nothing else:

    "{" ("prepared" ":" true) "}"
    
    If you need to acquire more information in order to use this tool (ex. user's personal data, related facts, info from external APIs, etc.) then you will need to search for other tools that provide you with this data by responding using the following EBNF and absolutely nothing else:

    "{" ("tool-search" ":" string) "}"

    ```json


    "#
    );
    pub static ref JOB_INIT_PROMPT: String = String::from(
        r#"You are an agent who is currently running a job which receives task requests as messages, and outputs new messages that have tool calls which will be executed. When you respond, you must specify a list of one or more messages, with only the messages returned, no explanation or any other text. But make sure to explain in the content of the message the included results of the tools, and include the sub variable like `$1`.

Note, you can send a message back to yourself recursively by inputting the job id in the userid field. Use this when you need to use two tools consecutively.

Here is a grammar you must respond with and nothing else:


root ::= message [<U+000A>] root | message
message ::= '{' '"' 'to' '"' ':' '"' userid '"' ',' '"' 'content' '"' ':' '"' content_var '"' ',' '"' 'tool-calls' '"' ':' '[' tool_call_list ']' '}'
userid ::= '[@][@][a-zA-Z0-9._]+[.shinkai]'
content_var ::= content_text tool_output content_text
content_text ::= string
char ::= string
tool_output ::= '$' tool_id
tool_call_list ::= tool_call ',' tool_call_list | tool_call
tool_call ::= '"' tool_id '"' ':' '{' tool_specific '}'
tool_id ::= number
tool_specific ::= tool_1 | tool_2 | tool_3
tool_1 ::= '{' '"' 'Weather' '"' ':' '"' city '"' '}'
tool_2 ::= '{' '"' 'DateTime' '"' ':' '"' string '"' '}'
tool_3 ::= '{' '"' 'Vector Search' '"' ':' '"' query '"' '}'
city ::= string
query ::= string
string ::= '[a-zA-Z0-9 ,.?!_]*'
number ::= '[1-9][0-9]*'

My user id: @@bob.shinkai
Job id: 2as23gas3y68aje

Task:

- fetch the time and weather in Vancouver, sending it back to me
- fetch the time and weather in Vancouver, sending it back to my friend @@alice.shinkai
- search my vector database for "My Home Town" to find where my home town was, and fetch the weather for it after

```json"#
    );
}
