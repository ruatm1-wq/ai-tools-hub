/// Chrome Browser Search via CDP (Chrome DevTools Protocol)
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;

type WS = WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

/// Search using Chrome browser, falls back gracefully if Chrome not available
pub async fn search(query: &str, port: u16) -> Result<String, String> {
    let ws_url = get_ws_url(port).await.map_err(|e| {
        format!("Chrome not available (start Chrome with --remote-debugging-port={}): {}", port, e)
    })?;

    let (mut ws, _) = connect_async(&ws_url).await.map_err(|e| format!("WS connect: {}", e))?;

    let result = do_search(&mut ws, query).await;
    let _ = ws.close(None).await;
    result
}

async fn do_search(ws: &mut WS, query: &str) -> Result<String, String> {
    // Create a new tab
    let target_id = cdp(ws, "Target.createTarget", json!({"url": "about:blank"})).await?;
    let tid = target_id["targetId"].as_str().ok_or("no targetId")?.to_string();

    // Attach to the target to get a session
    let attach = cdp(ws, "Target.attachToTarget", json!({"targetId": tid, "flatten": true})).await?;
    let sid = attach["sessionId"].as_str().ok_or("no sessionId")?.to_string();

    // Navigate to Google search
    let search_url = format!("https://www.google.com/search?q={}", urlencode(query));
    cdp_session(ws, &sid, "Page.enable", json!({})).await?;
    cdp_session(ws, &sid, "Page.navigate", json!({"url": search_url})).await?;

    // Wait for page load
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Extract page text
    let eval = cdp_session(ws, &sid, "Runtime.evaluate", json!({
        "expression": "document.body.innerText",
        "returnByValue": true
    })).await?;

    let text = eval["result"]["value"].as_str().unwrap_or("").to_string();

    // Clean up
    let _ = cdp(ws, "Target.closeTarget", json!({"targetId": tid})).await;

    if text.is_empty() {
        return Err("No content from page".to_string());
    }

    // Filter and clean
    let clean: Vec<&str> = text.lines()
        .map(|l| l.trim())
        .filter(|l| l.len() > 20 && !l.contains("Cookie") && !l.contains("Terms") && !l.contains("Privacy"))
        .collect();
    let result = clean.join("\n");
    let result = if result.len() > 3000 { format!("{}...[truncated]", &result[..3000]) } else { result };
    Ok(result)
}

/// CDP command (browser-level)
async fn cdp(ws: &mut WS, method: &str, params: Value) -> Result<Value, String> {
    let id = next_id();
    let cmd = json!({"id": id, "method": method, "params": params});
    ws.send(Message::Text(cmd.to_string())).await.map_err(|e| format!("Send: {:?}", e))?;
    read_response(ws, id).await
}

/// CDP command (session-level, for a specific tab)
async fn cdp_session(ws: &mut WS, session_id: &str, method: &str, params: Value) -> Result<Value, String> {
    let id = next_id();
    let cmd = json!({"id": id, "sessionId": session_id, "method": method, "params": params});
    ws.send(Message::Text(cmd.to_string())).await.map_err(|e| format!("Send: {:?}", e))?;
    read_response(ws, id).await
}

/// Read WS responses until we find the matching ID
async fn read_response(ws: &mut WS, expected_id: i64) -> Result<Value, String> {
    use std::time::{Duration, Instant};
    let start = Instant::now();

    while start.elapsed() < Duration::from_secs(15) {
        match tokio::time::timeout(Duration::from_millis(500), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(data) = serde_json::from_str::<Value>(&text) {
                    if data["id"].as_i64() == Some(expected_id) {
                        if data["error"].is_object() {
                            return Err(format!("CDP error: {:?}", data["error"]));
                        }
                        return Ok(data["result"].clone());
                    }
                }
            }
            Ok(Some(Ok(_))) => continue,
            Ok(None) => return Err("WS closed".to_string()),
            _ => continue,
        }
    }
    Err(format!("CDP timeout (id={})", expected_id))
}

fn next_id() -> i64 {
    static COUNTER: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

async fn get_ws_url(port: u16) -> Result<String, String> {
    let url = format!("http://localhost:{}/json/version", port);
    let resp = reqwest::get(&url).await.map_err(|e| format!("Chrome HTTP: {}", e))?;
    let data: Value = resp.json().await.map_err(|e| format!("Chrome JSON: {}", e))?;
    data["webSocketDebuggerUrl"].as_str().map(|s| s.to_string())
        .ok_or_else(|| "Chrome not running with --remote-debugging-port".to_string())
}

fn urlencode(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z'|'a'..='z'|'0'..='9'|'-'|'_'|'.'|'~' => c.to_string(),
        ' ' => "+".to_string(),
        _ => format!("%{:02X}", c as u8),
    }).collect()
}
