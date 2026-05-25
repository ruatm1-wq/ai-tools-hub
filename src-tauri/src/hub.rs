/// Hub Server: HTTP + WebSocket + Telegram Bridge + Agent Bridge
use axum::{
    Router, routing::{get, post},
    response::Json,
    extract::Query,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

// ── Types ──

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub source: String,
    pub reply_to: String,     // Telegram chat_id or empty
    pub timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatQuery {
    pub q: String,
    #[allow(dead_code)]
    pub tool_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HubConfig {
    pub telegram_token: String,
    pub vault_path: String,
}

impl Default for HubConfig {
    fn default() -> Self {
        HubConfig {
            telegram_token: String::new(),
            vault_path: Self::default_vault(),
        }
    }
}

impl HubConfig {
    fn default_vault() -> String {
        if cfg!(target_os = "windows") {
            r"D:\我的工作台".into()
        } else {
            dirs_data_dir().unwrap_or_else(|| ".".into())
        }
    }
}

fn dirs_data_dir() -> Option<String> {
    #[cfg(target_os = "windows")]
    { std::env::var("APPDATA").ok() }
    #[cfg(target_os = "linux")]
    { std::env::var("XDG_DATA_HOME").or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h))).ok() }
    #[cfg(target_os = "macos")]
    { std::env::var("HOME").map(|h| format!("{}/Library/Application Support", h)).ok() }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    { None }
}

// ── Agent Bridge ──

struct DeepSeekClient {
    api_key: String,
    api_base: String,
    model: String,
}

impl DeepSeekClient {
    fn from_env_or_default() -> Self {
        DeepSeekClient {
            api_key: std::env::var("DEEPSEEK_API_KEY").unwrap_or_default(),
            api_base: std::env::var("DEEPSEEK_API_BASE")
                .unwrap_or_else(|_| "https://api.deepseek.com".into()),
            model: std::env::var("DEEPSEEK_MODEL")
                .unwrap_or_else(|_| "deepseek-chat".into()),
        }
    }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn chat(&self, message: &str, system_prompt: &str) -> Result<String, String> {
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": message}
            ],
            "stream": false
        });

        let resp = client
            .post(format!("{}/v1/chat/completions", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("API error: {}", e))?;

        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse: {}", e))?;
        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("(no response)")
            .to_string();
        Ok(content)
    }
}

// ── Start Hub ──

pub fn start_hub() -> (broadcast::Sender<ChatMessage>, Arc<tokio::sync::Mutex<HubConfig>>) {
    let (tx, _rx) = broadcast::channel::<ChatMessage>(100);
    let tx_clone = tx.clone();
    let config = Arc::new(tokio::sync::Mutex::new(HubConfig::default()));
    let tx_tg = tx_clone.clone();
    let config_tg = config.clone();
    let config_tg2 = config.clone();

    // HTTP Server
    tokio::spawn(async move {
        let app = Router::new()
            .route("/health", get(|| async { Json(serde_json::json!({
                "status": "ok", "service": "ai-tools-hub", "version": "0.1.0"
            })) }))
            .route("/api/chat", post(handle_chat))
            .route("/webhook/receive", post(handle_webhook))
            .layer(tower_http::cors::CorsLayer::permissive());

        let port = try_bind(app).await;
        eprintln!("[hub] HTTP server on port {}", port);
    });

    // Telegram polling
    tokio::spawn(async move {
        telegram_poll_loop(tx_tg, config_tg).await;
    });

    // Agent worker: processes inbound messages from broadcast channel
    tokio::spawn(async move {
        agent_worker(tx_clone, config_tg2).await;
    });

    (tx, config)
}

async fn try_bind(app: Router) -> u16 {
    let addrs = ["0.0.0.0:27125", "0.0.0.0:27126", "0.0.0.0:27127", "0.0.0.0:0"];
    for addr in &addrs {
        if let Ok(l) = tokio::net::TcpListener::bind(addr).await {
            let port = l.local_addr().unwrap().port();
            tokio::spawn(async move { axum::serve(l, app).await.unwrap(); });
            return port;
        }
    }
    0
}

// ── Agent Worker ──
// Processes messages from the broadcast channel using the DeepSeek API.

async fn agent_worker(tx: broadcast::Sender<ChatMessage>, config: Arc<tokio::sync::Mutex<HubConfig>>) {
    let mut rx = tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if msg.role != "user" { continue; }
                eprintln!("[agent] Processing message from {}: {}", msg.source, &msg.content[..msg.content.len().min(50)]);

                let client = DeepSeekClient::from_env_or_default();
                if !client.is_configured() {
                    eprintln!("[agent] DEEPSEEK_API_KEY not set, skipping");
                    continue;
                }

                // Inject knowledge base context
                let kb_ctx = crate::knowledge::build_knowledge_context(&msg.content, 3);
                let sys_prompt = format!("You are a helpful AI assistant.{}", kb_ctx);

                match client.chat(&msg.content, &sys_prompt).await {
                    Ok(response) => {
                        // Broadcast response
                        let reply = ChatMessage {
                            id: format!("resp_{}", SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)),
                            role: "assistant".into(),
                            content: response.clone(),
                            source: "agent".into(),
                            reply_to: msg.reply_to.clone(),
                            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)
                                .map(|d| d.as_secs().to_string()).unwrap_or_default(),
                        };
                        let _ = tx.send(reply);

                        // If it came from Telegram, reply via Telegram
                        if msg.source.starts_with("telegram/") && !msg.reply_to.is_empty() {
                            if let Ok(chat_id) = msg.reply_to.parse::<i64>() {
                                let cfg = config.lock().await;
                                let _ = send_telegram(chat_id, &response, &cfg).await;
                            }
                        }
                    }
                    Err(e) => eprintln!("[agent] Chat error: {}", e),
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("[agent] Lagged {} messages", n);
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

// ── Handlers ──

async fn handle_chat(
    Query(params): Query<ChatQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = DeepSeekClient::from_env_or_default();
    if !client.is_configured() {
        return Ok(Json(serde_json::json!({
            "error": "DEEPSEEK_API_KEY not configured. Set it in your environment or via hub config."
        })));
    }

    let kb_ctx = crate::knowledge::build_knowledge_context(&params.q, 3);
    let sys_prompt = format!("You are a helpful AI assistant accessible via HTTP API.{}", kb_ctx);
    match client.chat(&params.q, &sys_prompt).await {
        Ok(response) => Ok(Json(serde_json::json!({
            "response": response,
            "status": "ok"
        }))),
        Err(e) => Ok(Json(serde_json::json!({
            "error": e,
            "status": "error"
        }))),
    }
}

async fn handle_webhook(
    body: String,
) -> StatusCode {
    eprintln!("[hub] Webhook: {} bytes", body.len());
    StatusCode::OK
}

// ── Telegram ──

async fn telegram_poll_loop(tx: broadcast::Sender<ChatMessage>, config: Arc<tokio::sync::Mutex<HubConfig>>) {
    let client = reqwest::Client::new();

    loop {
        let token = { config.lock().await.telegram_token.clone() };
        if token.is_empty() {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            continue;
        }

        let api_base = format!("https://api.telegram.org/bot{}", token);
        let mut offset: i64 = 0;
        eprintln!("[tg] Polling started");

        loop {
            let current_token = { config.lock().await.telegram_token.clone() };
            if current_token != token { break; }

            let resp = client
                .post(format!("{}/getUpdates", api_base))
                .json(&serde_json::json!({
                    "offset": offset,
                    "timeout": 30,
                    "allowed_updates": ["message"]
                }))
                .send().await;

            match resp {
                Ok(r) => {
                    if let Ok(data) = r.json::<serde_json::Value>().await {
                        if let Some(updates) = data["result"].as_array() {
                            for update in updates {
                                if let Some(upd_id) = update["update_id"].as_i64() {
                                    offset = upd_id + 1;
                                }
                                if let Some(msg) = update["message"].as_object() {
                                    let chat_id = msg["chat"]["id"].as_i64().unwrap_or(0);
                                    let text = msg["text"].as_str().unwrap_or("").to_string();
                                    let from = msg["from"]["first_name"].as_str().unwrap_or("User").to_string();

                                    if !text.is_empty() && !text.starts_with('/') {
                                        let payload = ChatMessage {
                                            id: format!("tg_{}", update["update_id"].as_i64().unwrap_or(0)),
                                            role: "user".into(),
                                            content: text.clone(),
                                            source: format!("telegram/{}", from),
                                            reply_to: chat_id.to_string(),
                                            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)
                                                .map(|d| d.as_secs().to_string()).unwrap_or_default(),
                                        };
                                        let _ = tx.send(payload);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[tg] Poll error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
}

/// Send a message to Telegram
pub async fn send_telegram(chat_id: i64, text: &str, config: &HubConfig) -> Result<(), String> {
    if config.telegram_token.is_empty() {
        return Err("Telegram not configured".into());
    }
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("https://api.telegram.org/bot{}/sendMessage", config.telegram_token))
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown"
        }))
        .send().await.map_err(|e| e.to_string())?;

    if resp.status().is_success() { Ok(()) }
    else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("Telegram error: {}", body))
    }
}
