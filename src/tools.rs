use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use std::env;

use crate::protocol::*;

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
}
