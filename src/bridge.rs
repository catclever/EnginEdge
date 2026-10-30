use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::StreamExt;
use url::Url;
use tracing::{info, error, debug, warn};

#[derive(Clone)]
pub struct CloudBridge {
    is_connected: Arc<RwLock<bool>>,
    hub_url: Option<String>,
}

impl CloudBridge {
    pub fn new(hub_url: Option<String>) -> Self {
        Self {
            is_connected: Arc::new(RwLock::new(false)),
            hub_url,
        }
    }

    pub fn start(&self) {
        if let Some(url_str) = &self.hub_url {
            let url_str = url_str.clone();
            let is_connected = self.is_connected.clone();

            tokio::spawn(async move {
                Self::run_connection_loop(url_str, is_connected).await;
            });
        } else {
            info!("No hub URL provided. Cloud Bridge is disabled.");
        }
    }

    async fn run_connection_loop(url_str: String, is_connected: Arc<RwLock<bool>>) {
        loop {
            match Url::parse(&url_str) {
                Ok(url) => {
                    info!("Attempting to connect to Cloud Bridge at {}", url);
                    match connect_async(url.as_str()).await {
                        Ok((ws_stream, _)) => {
                            info!("Successfully connected to Cloud Bridge");
                            *is_connected.write().await = true;

                            let (_write, mut read) = ws_stream.split();

                            // Read loop
                            while let Some(msg_result) = read.next().await {
                                match msg_result {
                                    Ok(Message::Close(_)) => {
                                        warn!("Cloud Bridge connection closed by server");
                                        break;
                                    }
                                    Ok(msg) => {
                                        debug!("Received message from Cloud Bridge: {:?}", msg);
                                    }
                                    Err(e) => {
                                        error!("Error reading from Cloud Bridge: {}", e);
                                        break;
                                    }
                                }
                            }

                            warn!("Disconnected from Cloud Bridge");
                            *is_connected.write().await = false;
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

            // Reconnection delay
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }
}
