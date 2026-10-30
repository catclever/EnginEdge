use serde::{Deserialize, Serialize};
use serde_json::Value;

// JSON-RPC 2.0 Base Structures

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Id {
    String(String),
    Number(i64),
    Null,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Id,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Id,
    pub result: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: String,
    pub id: Id,
    pub error: JsonRpcError,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// MCP Specific Structures

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    pub client_info: Implementation,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub server_info: Implementation,
    pub capabilities: ServerCapabilities,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    pub tools: Option<Value>, // Can be empty object for capabilities
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolListResult {
    pub tools: Vec<Tool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CallToolRequestParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    Text { text: String },
    // Other content types like Image can be added later
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_request() {
        let req_str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1.0","clientInfo":{"name":"test","version":"0.1"}}}"#;
        let req: JsonRpcRequest = serde_json::from_str(req_str).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, Id::Number(1));
        assert_eq!(req.method, "initialize");
    }

    #[test]
    fn test_serialize_response() {
        let res = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Id::String("abc".to_string()),
            result: json!({"status": "ok"}),
        };
        let res_str = serde_json::to_string(&res).unwrap();
        assert_eq!(res_str, r#"{"jsonrpc":"2.0","id":"abc","result":{"status":"ok"}}"#);
    }

    #[test]
    fn test_serialize_error() {
        let err = JsonRpcErrorResponse {
            jsonrpc: "2.0".to_string(),
            id: Id::Number(1),
            error: JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            },
        };
        let err_str = serde_json::to_string(&err).unwrap();
        assert_eq!(err_str, r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#);
    }
}
