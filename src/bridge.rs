use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{SinkExt, StreamExt};
use url::Url;
use tracing::{debug, error, info, warn};
use serde_json::{json, Value};
use crate::protocol::JsonRpcError;

#[derive(Clone, Debug)]
pub enum BridgeEvent {
    StreamEvent { event: String, data: Value },
    Finished { result: Value, status: String },
    Error { message: String },
}

#[derive(Clone)]
pub struct CloudBridge {
    is_connected: Arc<RwLock<bool>>,
    hub_url: Option<String>,
    outgoing_tx: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>>,
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
    event_tx: broadcast::Sender<BridgeEvent>,
}

impl CloudBridge {
    pub fn new(hub_url: Option<String>) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            is_connected: Arc::new(RwLock::new(false)),
            hub_url,
            outgoing_tx: Arc::new(Mutex::new(None)),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<BridgeEvent> {
        self.event_tx.subscribe()
    }

    pub fn start(&self) {
        if let Some(url_str) = &self.hub_url {
            let url_str = url_str.clone();
            let is_connected = self.is_connected.clone();
            let outgoing_tx = self.outgoing_tx.clone();
            let pending_requests = self.pending_requests.clone();
            let event_tx = self.event_tx.clone();

            tokio::spawn(async move {
                Self::run_connection_loop(url_str, is_connected, outgoing_tx, pending_requests, event_tx).await;
            });
        } else {
            info!("No hub URL provided. Cloud Bridge is disabled.");
        }
    }

    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    pub async fn wait_for_connection(&self, timeout: Duration) -> bool {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if self.is_connected().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        false
    }

    pub async fn send_raw(&self, msg: String) -> Result<(), String> {
        let lock = self.outgoing_tx.lock().await;
        if let Some(tx) = lock.as_ref() {
            tx.send(msg).map_err(|e| format!("Failed to send raw message: {}", e))
        } else {
            Err("Cloud bridge is not connected".to_string())
        }
    }

    pub async fn call_remote_tool(&self, tool_name: &str, args: Value) -> Result<String, JsonRpcError> {
        if !self.is_connected().await {
            return Err(JsonRpcError {
                code: -32000,
                message: "Cloud bridge is disconnected".to_string(),
                data: None,
            });
        }

        let call_id = format!("call_{}_{}", tool_name, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(call_id.clone(), tx);
        }

        let payload = json!({
            "type": "execute_tool",
            "call_id": call_id,
            "tool": tool_name,
            "arguments": args
        });

        if let Err(e) = self.send_raw(payload.to_string()).await {
            let mut pending = self.pending_requests.lock().await;
            pending.remove(&call_id);
            return Err(JsonRpcError {
                code: -32603,
                message: format!("Failed to send tool request to cloud: {}", e),
                data: None,
            });
        }

        match tokio::time::timeout(Duration::from_secs(60), rx).await {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(_)) => Err(JsonRpcError {
                code: -32603,
                message: "Cloud bridge connection closed while awaiting tool result".to_string(),
                data: None,
            }),
            Err(_) => {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&call_id);
                Err(JsonRpcError {
                    code: -32603,
                    message: "Cloud tool execution timed out after 60s".to_string(),
                    data: None,
                })
            }
        }
    }

    async fn run_connection_loop(
        url_str: String,
        is_connected: Arc<RwLock<bool>>,
        outgoing_tx: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>>,
        pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
        event_tx: broadcast::Sender<BridgeEvent>,
    ) {
        loop {
            match Url::parse(&url_str) {
                Ok(mut url) => {
                    // Check and inject HUB_AUTH_TOKEN / DESCARTES_AUTH_TOKEN if available
                    if let Ok(token) = std::env::var("HUB_AUTH_TOKEN").or_else(|_| std::env::var("DESCARTES_AUTH_TOKEN")) {
                        let token = token.trim();
                        if !token.is_empty() {
                            let mut pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
                            if !pairs.iter().any(|(k, _)| k == "token") {
                                pairs.push(("token".to_string(), token.to_string()));
                                url.query_pairs_mut().clear().extend_pairs(pairs);
                            }
                        }
                    }

                    info!("Attempting to connect to Cloud Bridge at {}", url);
                    match connect_async(url.as_str()).await {
                        Ok((ws_stream, _)) => {
                            info!("Successfully connected to Cloud Bridge");
                            *is_connected.write().await = true;

                            let (mut ws_writer, mut ws_reader) = ws_stream.split();
                            let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                            *outgoing_tx.lock().await = Some(tx.clone());

                            // Writer task
                            let writer_handle = tokio::spawn(async move {
                                while let Some(msg) = rx.recv().await {
                                    if let Err(e) = ws_writer.send(Message::Text(msg.into())).await {
                                        error!("Error sending message to Cloud Bridge: {}", e);
                                        break;
                                    }
                                }
                            });

                            // Reader loop
                            while let Some(msg_result) = ws_reader.next().await {
                                match msg_result {
                                    Ok(Message::Text(text)) => {
                                        debug!("Received message from Cloud Bridge: {}", text);
                                        if let Ok(val) = serde_json::from_str::<Value>(&text) {
                                            let msg_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                            match msg_type {
                                                "tool_result" => {
                                                    if let Some(call_id) = val.get("call_id").and_then(|c| c.as_str()) {
                                                        let output = val.get("output")
                                                            .and_then(|o| o.as_str())
                                                            .map(|s| s.to_string())
                                                            .unwrap_or_else(|| val.get("output").map(|o| o.to_string()).unwrap_or_default());
                                                        if let Some(sender) = pending_requests.lock().await.remove(call_id) {
                                                            let _ = sender.send(output);
                                                        }
                                                    }
                                                }
                                                "execute_tool" => {
                                                    let call_id = val.get("call_id").and_then(|c| c.as_str()).unwrap_or("").to_string();
                                                    let tool_name = val.get("tool").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                                    let arguments = val.get("arguments").cloned().unwrap_or(json!({}));

                                                    let tx_clone = tx.clone();
                                                    tokio::spawn(async move {
                                                        let output = crate::tools::execute_local_tool_by_name(&tool_name, arguments).await;
                                                        let reply = json!({
                                                            "type": "tool_result",
                                                            "call_id": call_id,
                                                            "output": output
                                                        });
                                                        let _ = tx_clone.send(reply.to_string());
                                                    });
                                                }
                                                "stream_event" => {
                                                    let event = val.get("event").and_then(|e| e.as_str()).unwrap_or("").to_string();
                                                    let data = val.get("data").cloned().unwrap_or(json!({}));
                                                    let _ = event_tx.send(BridgeEvent::StreamEvent { event, data });
                                                }
                                                "finished" => {
                                                    let result = val.get("result").cloned().unwrap_or(json!({}));
                                                    let status = val.get("status").and_then(|s| s.as_str()).unwrap_or("ok").to_string();
                                                    let _ = event_tx.send(BridgeEvent::Finished { result, status });
                                                }
                                                "error" => {
                                                    let message = val.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown cloud error").to_string();
                                                    let _ = event_tx.send(BridgeEvent::Error { message });
                                                }
                                                _ => {
                                                    debug!("Ignored message type: {}", msg_type);
                                                }
                                            }
                                        }
                                    }
                                    Ok(Message::Close(frame)) => {
                                        warn!("Cloud Bridge closed by server: {:?}", frame);
                                        break;
                                    }
                                    Ok(Message::Ping(_)) => {}
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("Error reading from Cloud Bridge: {}", e);
                                        break;
                                    }
                                }
                            }

                            writer_handle.abort();
                            warn!("Disconnected from Cloud Bridge");
                            *is_connected.write().await = false;
                            *outgoing_tx.lock().await = None;

                            // Drain pending requests
                            let mut pending = pending_requests.lock().await;
                            for (_, sender) in pending.drain() {
                                let _ = sender.send("Error: Cloud Bridge disconnected".to_string());
                            }

                            // CRITICAL: Notify subscribers (e.g. main.rs headless runner) to prevent deadlock
                            let _ = event_tx.send(BridgeEvent::Error {
                                message: "Disconnected from Cloud Bridge".to_string(),
                            });
                        }
                        Err(e) => {
                            error!("Failed to connect to Cloud Bridge: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Invalid hub URL: {}", e);
                    break;
                }
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
