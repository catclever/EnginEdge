use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};
use serde_json::{json, Value};
use tracing::{debug, error, info};

use crate::protocol::*;
use crate::tools;
use crate::bridge::CloudBridge;

pub async fn run_stdio_server(bridge: CloudBridge) -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    // We send responses via standard stdout synchronously for simplicity,
    // though in a fully async environment we could use tokio::io::stdout().
    // But stdout must be EXCLUSIVELY protocol lines.

    info!("Server listening on stdio");

    while let Some(line) = reader.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        debug!("Received: {}", line);

        match serde_json::from_str::<Value>(line) {
            Ok(value) => {
                // Determine if it's a request or notification
                if let Some(id_val) = value.get("id") {
                    // It's a request
                    if let Ok(request) = serde_json::from_value::<JsonRpcRequest>(value.clone()) {
                        handle_request(request, &bridge).await;
                    } else {
                        // Malformed request, send standard error if id can be extracted
                        let id = serde_json::from_value::<Id>(id_val.clone()).unwrap_or(Id::Null);
                        send_error(id, -32600, "Invalid Request");
                    }
                } else {
                    // Notification
                    if let Ok(notification) = serde_json::from_value::<JsonRpcNotification>(value) {
                        handle_notification(notification).await;
                    }
                }
            }
            Err(e) => {
                error!("Parse error: {}", e);
                send_error(Id::Null, -32700, "Parse error");
            }
        }
    }

    info!("Server shut down");
    Ok(())
}

async fn handle_request(req: JsonRpcRequest, bridge: &CloudBridge) {
    match req.method.as_str() {
        "initialize" => {
            let result = InitializeResult {
                protocol_version: "2024-11-05".to_string(), // Arbitrary recent version
                server_info: Implementation {
                    name: "enginedge".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                capabilities: ServerCapabilities {
                    tools: Some(json!({})),
                },
            };
            send_response(req.id, json!(result));
        }
        "tools/list" => {
            let result = ToolListResult {
                tools: vec![
                    tools::ping_tool_def(),
                    tools::echo_tool_def(),
                    tools::get_status_tool_def(),
                    tools::get_outline_tool_def(),
                    tools::read_symbol_tool_def(),
                    tools::replace_symbol_tool_def(),
                    tools::survey_architecture_tool_def(),
                    tools::search_code_examples_tool_def(),
                    tools::fetch_doc_tool_def(),
                    tools::inspect_spec_tool_def(),
                ],
            };
            send_response(req.id, json!(result));
        }
        "tools/call" => {
            if let Some(params) = req.params {
                if let Ok(call_params) = serde_json::from_value::<CallToolRequestParams>(params) {
                    let result = match call_params.name.as_str() {
                        "ping" => tools::execute_ping(),
                        "echo" => tools::execute_echo(call_params.arguments.clone()),
                        "get_status" => tools::execute_get_status(),
                        "get_outline" => tools::execute_get_outline(call_params.arguments),
                        "read_symbol" => tools::execute_read_symbol(call_params.arguments),
                        "replace_symbol" => tools::execute_replace_symbol(call_params.arguments),
                        "survey_architecture" => tools::execute_survey_architecture(call_params.arguments, bridge).await,
                        "search_code_examples" => tools::execute_search_code_examples(call_params.arguments, bridge).await,
                        "fetch_doc" => tools::execute_fetch_doc(call_params.arguments, bridge).await,
                        "inspect_spec" => tools::execute_inspect_spec(call_params.arguments, bridge).await,
                        _ => Err(JsonRpcError {
                            code: -32601,
                            message: format!("Tool not found: {}", call_params.name),
                            data: None,
                        }),
                    };

                    match result {
                        Ok(res) => send_response(req.id, json!(res)),
                        Err(e) => send_custom_error(req.id, e),
                    }
                } else {
                    send_error(req.id, -32602, "Invalid params for tools/call");
                }
            } else {
                send_error(req.id, -32602, "Missing params for tools/call");
            }
        }
        _ => {
            send_error(req.id, -32601, "Method not found");
        }
    }
}

async fn handle_notification(notif: JsonRpcNotification) {
    match notif.method.as_str() {
        "notifications/initialized" => {
            debug!("Client initialized");
        }
        _ => {
            debug!("Unknown notification: {}", notif.method);
        }
    }
}

fn send_response(id: Id, result: Value) {
    let res = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result,
    };
    send_out(res);
}

fn send_error(id: Id, code: i64, message: &str) {
    let err = JsonRpcErrorResponse {
        jsonrpc: "2.0".to_string(),
        id,
        error: JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
        },
    };
    send_out(err);
}

fn send_custom_error(id: Id, error: JsonRpcError) {
    let err = JsonRpcErrorResponse {
        jsonrpc: "2.0".to_string(),
        id,
        error,
    };
    send_out(err);
}

fn send_out<T: serde::Serialize>(msg: T) {
    let out = serde_json::to_string(&msg).unwrap();
    debug!("Sending: {}", out);
    // CRITICAL: stdout must ONLY contain valid protocol lines.
    let mut stdout = io::stdout();
    let _ = writeln!(stdout, "{}", out);
    let _ = stdout.flush();
}
