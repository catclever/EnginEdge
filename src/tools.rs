use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use std::env;

use crate::protocol::*;
use crate::ast;
use crate::bridge::CloudBridge;

pub fn get_outline_tool_def() -> Tool {
    Tool {
        name: "get_outline".to_string(),
        description: Some("Extracts high-level symbols (functions, structs, etc) from a file".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string"
                }
            },
            "required": ["file_path"]
        }),
    }
}

pub fn execute_get_outline(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    if let Some(args_obj) = args {
        if let Some(file_path) = args_obj.get("file_path").and_then(|m| m.as_str()) {
            match ast::get_outline(file_path) {
                Ok(outline) => {
                    return Ok(CallToolResult {
                        content: vec![Content::Text {
                            text: outline.to_string(),
                        }],
                        is_error: None,
                    });
                }
                Err(e) => {
                    return Err(JsonRpcError {
                        code: -32603,
                        message: format!("Failed to get outline: {}", e),
                        data: None,
                    });
                }
            }
        }
    }

    Err(JsonRpcError {
        code: -32602,
        message: "Missing required argument 'file_path'".to_string(),
        data: None,
    })
}

pub fn read_symbol_tool_def() -> Tool {
    Tool {
        name: "read_symbol".to_string(),
        description: Some("Extracts the exact code block for a specific target symbol".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string"
                },
                "symbol_path": {
                    "type": "string"
                }
            },
            "required": ["file_path", "symbol_path"]
        }),
    }
}

pub fn execute_read_symbol(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    if let Some(args_obj) = args {
        let file_path = args_obj.get("file_path").and_then(|m| m.as_str());
        let symbol_path = args_obj.get("symbol_path").and_then(|m| m.as_str());

        if let (Some(fp), Some(sp)) = (file_path, symbol_path) {
            match ast::read_symbol(fp, sp) {
                Ok(symbol) => {
                    return Ok(CallToolResult {
                        content: vec![Content::Text {
                            text: symbol.to_string(),
                        }],
                        is_error: None,
                    });
                }
                Err(e) => {
                    return Err(JsonRpcError {
                        code: -32603,
                        message: format!("Failed to read symbol: {}", e),
                        data: None,
                    });
                }
            }
        }
    }

    Err(JsonRpcError {
        code: -32602,
        message: "Missing required arguments 'file_path' and 'symbol_path'".to_string(),
        data: None,
    })
}

pub fn replace_symbol_tool_def() -> Tool {
    Tool {
        name: "replace_symbol".to_string(),
        description: Some("Atomically replaces the target AST node's span with new_code".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string"
                },
                "symbol_path": {
                    "type": "string"
                },
                "new_code": {
                    "type": "string"
                }
            },
            "required": ["file_path", "symbol_path", "new_code"]
        }),
    }
}

pub fn execute_replace_symbol(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    if let Some(args_obj) = args {
        let file_path = args_obj.get("file_path").and_then(|m| m.as_str());
        let symbol_path = args_obj.get("symbol_path").and_then(|m| m.as_str());
        let new_code = args_obj.get("new_code").and_then(|m| m.as_str());

        if let (Some(fp), Some(sp), Some(nc)) = (file_path, symbol_path, new_code) {
            match ast::replace_symbol(fp, sp, nc) {
                Ok(result) => {
                    return Ok(CallToolResult {
                        content: vec![Content::Text {
                            text: result.to_string(),
                        }],
                        is_error: None,
                    });
                }
                Err(e) => {
                    return Err(JsonRpcError {
                        code: -32603,
                        message: format!("Failed to replace symbol: {}", e),
                        data: None,
                    });
                }
            }
        }
    }

    Err(JsonRpcError {
        code: -32602,
        message: "Missing required arguments 'file_path', 'symbol_path', 'new_code'".to_string(),
        data: None,
    })
}

pub fn ping_tool_def() -> Tool {
    Tool {
        name: "ping".to_string(),
        description: Some("Returns pong and the current timestamp".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn execute_ping() -> Result<CallToolResult, JsonRpcError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: format!("pong: {}", now),
        }],
        is_error: None,
    })
}

// Cloud Intelligence Tools

pub fn survey_architecture_tool_def() -> Tool {
    Tool {
        name: "survey_architecture".to_string(),
        description: Some("Discovers open-source repo blueprints".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "topic": { "type": "string" }
            },
            "required": ["query"]
        }),
    }
}

pub async fn execute_survey_architecture(_args: Option<Value>, bridge: &CloudBridge) -> Result<CallToolResult, JsonRpcError> {
    if !bridge.is_connected().await {
        return Err(JsonRpcError {
            code: -32000,
            message: "Cloud bridge is disconnected".to_string(),
            data: None,
        });
    }

    Ok(CallToolResult {
        content: vec![Content::Text { text: "Simulated architecture survey result".to_string() }],
        is_error: None,
    })
}

pub fn search_code_examples_tool_def() -> Tool {
    Tool {
        name: "search_code_examples".to_string(),
        description: Some("Retrieves real-world production code snippets".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "lang": { "type": "string" }
            },
            "required": ["query"]
        }),
    }
}

pub async fn execute_search_code_examples(_args: Option<Value>, bridge: &CloudBridge) -> Result<CallToolResult, JsonRpcError> {
    if !bridge.is_connected().await {
        return Err(JsonRpcError {
            code: -32000,
            message: "Cloud bridge is disconnected".to_string(),
            data: None,
        });
    }

    Ok(CallToolResult {
        content: vec![Content::Text { text: "Simulated code examples result".to_string() }],
        is_error: None,
    })
}

pub fn fetch_doc_tool_def() -> Tool {
    Tool {
        name: "fetch_doc".to_string(),
        description: Some("Pulls sanitized Markdown documentation".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "package_or_url": { "type": "string" }
            },
            "required": ["package_or_url"]
        }),
    }
}

pub async fn execute_fetch_doc(_args: Option<Value>, bridge: &CloudBridge) -> Result<CallToolResult, JsonRpcError> {
    if !bridge.is_connected().await {
        return Err(JsonRpcError {
            code: -32000,
            message: "Cloud bridge is disconnected".to_string(),
            data: None,
        });
    }

    Ok(CallToolResult {
        content: vec![Content::Text { text: "Simulated documentation result".to_string() }],
        is_error: None,
    })
}

pub fn inspect_spec_tool_def() -> Tool {
    Tool {
        name: "inspect_spec".to_string(),
        description: Some("Retrieves test specs as executable living documentation".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string" },
                "symbol": { "type": "string" }
            },
            "required": ["repo"]
        }),
    }
}

pub async fn execute_inspect_spec(_args: Option<Value>, bridge: &CloudBridge) -> Result<CallToolResult, JsonRpcError> {
    if !bridge.is_connected().await {
        return Err(JsonRpcError {
            code: -32000,
            message: "Cloud bridge is disconnected".to_string(),
            data: None,
        });
    }

    Ok(CallToolResult {
        content: vec![Content::Text { text: "Simulated test spec result".to_string() }],
        is_error: None,
    })
}

pub fn echo_tool_def() -> Tool {
    Tool {
        name: "echo".to_string(),
        description: Some("Echoes the provided message".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string"
                }
            },
            "required": ["message"]
        }),
    }
}

pub fn execute_echo(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    if let Some(args_obj) = args {
        if let Some(msg) = args_obj.get("message").and_then(|m| m.as_str()) {
            return Ok(CallToolResult {
                content: vec![Content::Text {
                    text: msg.to_string(),
                }],
                is_error: None,
            });
        }
    }

    Err(JsonRpcError {
        code: -32602,
        message: "Missing required argument 'message'".to_string(),
        data: None,
    })
}

pub fn get_status_tool_def() -> Tool {
    Tool {
        name: "get_status".to_string(),
        description: Some("Returns server status, current directory, and version".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn execute_get_status() -> Result<CallToolResult, JsonRpcError> {
    let version = env!("CARGO_PKG_VERSION");
    let current_dir = env::current_dir()
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let status_info = json!({
        "status": "running",
        "version": version,
        "current_directory": current_dir,
    });

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: status_info.to_string(),
        }],
        is_error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo_success() {
        let args = json!({"message": "hello world"});
        let result = execute_echo(Some(args)).unwrap();
        assert_eq!(result.content.len(), 1);
        if let Content::Text { text } = &result.content[0] {
            assert_eq!(text, "hello world");
        } else {
            panic!("Expected text content");
        }
    }

    #[test]
    fn test_echo_missing_args() {
        let result = execute_echo(None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn test_ping_execution() {
        let result = execute_ping().unwrap();
        if let Content::Text { text } = &result.content[0] {
            assert!(text.starts_with("pong: "));
        } else {
            panic!("Expected text content");
        }
    }

    #[test]
    fn test_get_status_execution() {
        let result = execute_get_status().unwrap();
        if let Content::Text { text } = &result.content[0] {
            let parsed: Value = serde_json::from_str(text).unwrap();
            assert_eq!(parsed.get("status").unwrap().as_str().unwrap(), "running");
            assert!(parsed.get("version").is_some());
            assert!(parsed.get("current_directory").is_some());
        } else {
            panic!("Expected text content");
        }
    }

    #[tokio::test]
    async fn test_cloud_tools_offline_fallback() {
        let bridge = CloudBridge::new(None); // no URL, won't connect

        // Check survey_architecture offline fallback
        let survey_res = execute_survey_architecture(Some(json!({"query": "test"})), &bridge).await;
        assert!(survey_res.is_err());
        let err = survey_res.unwrap_err();
        assert_eq!(err.code, -32000);
        assert_eq!(err.message, "Cloud bridge is disconnected");

        // Check search_code_examples offline fallback
        let search_res = execute_search_code_examples(Some(json!({"query": "test", "lang": "rust"})), &bridge).await;
        assert!(search_res.is_err());
        assert_eq!(search_res.unwrap_err().code, -32000);

        // Check fetch_doc offline fallback
        let fetch_res = execute_fetch_doc(Some(json!({"package_or_url": "test"})), &bridge).await;
        assert!(fetch_res.is_err());
        assert_eq!(fetch_res.unwrap_err().code, -32000);

        // Check inspect_spec offline fallback
        let inspect_res = execute_inspect_spec(Some(json!({"repo": "test"})), &bridge).await;
        assert!(inspect_res.is_err());
        assert_eq!(inspect_res.unwrap_err().code, -32000);
    }
}
