use lazy_static::lazy_static;

lazy_static! {
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
