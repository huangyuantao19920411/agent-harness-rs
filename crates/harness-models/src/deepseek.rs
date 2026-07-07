//! DeepSeek API adapter (OpenAI-compatible chat completions).

use async_trait::async_trait;
use harness_core::{
    CompletionRequest, CompletionResult, Message, ModelProvider, ModelResponse, Role,
    ToolCallRef, ToolSchemaRef,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
const DEFAULT_MODEL: &str = "deepseek-chat";

/// DeepSeek chat model via OpenAI-compatible API.
#[derive(Clone)]
pub struct DeepSeekModel {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl DeepSeekModel {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.into(),
            model: DEFAULT_MODEL.into(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("DEEPSEEK_API_KEY").ok()?;
        let mut model = Self::new(api_key);
        if let Ok(name) = std::env::var("DEEPSEEK_MODEL") {
            model = model.with_model(name);
        }
        if let Ok(base_url) = std::env::var("DEEPSEEK_BASE_URL") {
            model = model.with_base_url(base_url);
        }
        Some(model)
    }

    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    pub(crate) fn build_payload(&self, request: &CompletionRequest) -> ChatRequest {
        let messages = request
            .messages
            .iter()
            .map(to_api_message)
            .collect::<Vec<_>>();

        let tools = if request.tools.is_empty() {
            None
        } else {
            Some(
                request
                    .tools
                    .iter()
                    .map(to_api_tool)
                    .collect::<Vec<_>>(),
            )
        };

        let tool_choice = tools.as_ref().map(|_| json!("auto"));

        ChatRequest {
            model: self.model.clone(),
            messages,
            tools,
            tool_choice,
            stream: false,
        }
    }
}

#[async_trait]
impl ModelProvider for DeepSeekModel {
    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<CompletionResult, String> {
        let payload = self.build_payload(&request);

        let response = self
            .client
            .post(self.chat_url())
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| format!("read body failed: {e}"))?;

        if !status.is_success() {
            return Err(format!("DeepSeek API error ({status}): {body}"));
        }

        let parsed: ChatResponse =
            serde_json::from_str(&body).map_err(|e| format!("invalid JSON: {e}; body={body}"))?;

        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| "empty choices in response".to_string())?;

        let message = choice.message;

        if let Some(tool_calls) = message.tool_calls {
            let calls: Vec<ToolCallRef> = tool_calls
                .into_iter()
                .map(|tc| ToolCallRef {
                    id: tc.id,
                    name: tc.function.name,
                    arguments: serde_json::from_str(&tc.function.arguments)
                        .unwrap_or_else(|_| json!({ "raw": tc.function.arguments })),
                })
                .collect();

            if !calls.is_empty() {
                return Ok(CompletionResult {
                    response: ModelResponse::ToolCalls(calls),
                });
            }
        }

        let content = message.content.unwrap_or_default();
        Ok(CompletionResult {
            response: ModelResponse::Text(content),
        })
    }
}

fn to_api_message(message: &Message) -> ApiMessage {
    let role = match message.role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    };

    let mut api = ApiMessage {
        role: role.into(),
        content: if message.content.is_empty() {
            None
        } else {
            Some(message.content.clone())
        },
        tool_call_id: message.tool_call_id.clone(),
        tool_calls: None,
    };

    if let Some(calls) = &message.tool_calls {
        api.tool_calls = Some(
            calls
                .iter()
                .map(|c| ApiToolCall {
                    id: c.id.clone(),
                    call_type: "function".into(),
                    function: ApiFunctionCall {
                        name: c.name.clone(),
                        arguments: c.arguments.clone(),
                    },
                })
                .collect(),
        );
    }

    api
}

fn to_api_tool(tool: &ToolSchemaRef) -> ApiToolDefinition {
    ApiToolDefinition {
        tool_type: "function".into(),
        function: ApiFunctionSchema {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.parameters.clone(),
        },
    }
}

#[derive(Serialize)]
pub(crate) struct ChatRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
    stream: bool,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Serialize)]
struct ApiToolDefinition {
    #[serde(rename = "type")]
    tool_type: String,
    function: ApiFunctionSchema,
}

#[derive(Serialize)]
struct ApiFunctionSchema {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Serialize)]
struct ApiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: ApiFunctionCall,
}

#[derive(Serialize)]
struct ApiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ApiResponseMessage,
}

#[derive(Deserialize)]
struct ApiResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ApiResponseToolCall>>,
}

#[derive(Deserialize)]
struct ApiResponseToolCall {
    id: String,
    function: ApiResponseFunction,
}

#[derive(Deserialize)]
struct ApiResponseFunction {
    name: String,
    arguments: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_openai_compatible_payload() {
        let model = DeepSeekModel::new("test-key");
        let payload = model.build_payload(&CompletionRequest {
            messages: vec![Message::user("hello")],
            tools: vec![ToolSchemaRef {
                name: "list_dir".into(),
                description: "List directory".into(),
                parameters: json!({"type": "object"}),
            }],
        });

        assert_eq!(payload.model, "deepseek-chat");
        assert_eq!(payload.messages.len(), 1);
        assert!(payload.tools.is_some());
    }
}
