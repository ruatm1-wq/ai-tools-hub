/// CLI entry point — instant terminal chat, no GUI needed.
/// Usage: ai-tools-hub --cli
use std::io::{self, Write};

#[allow(dead_code)]
pub async fn run_cli() {
    eprintln!("[cli] AI Tools Hub — Terminal Mode");
    eprintln!("[cli] Type your message, or /quit to exit.");

    let _stdin = io::BufReader::new(io::stdin());
    let mut input = String::new();

    loop {
        print!("\n> ");
        io::stdout().flush().ok();
        input.clear();
        io::stdin().read_line(&mut input).ok();
        let msg = input.trim();

        if msg.is_empty() { continue; }
        if msg == "/quit" || msg == "/exit" { break; }
        if msg.starts_with('/') {
            eprintln!("[cli] Unknown command: {}", msg);
            continue;
        }

        // Quick chat using DeepSeek directly
        let api_key = std::env::var("DEEPSEEK_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            eprintln!("[cli] Set DEEPSEEK_API_KEY first");
            continue;
        }

        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "model": "deepseek-chat",
            "messages": [{"role": "user", "content": msg}],
            "stream": false
        });

        match client
            .post("https://api.deepseek.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(content) = data["choices"][0]["message"]["content"].as_str() {
                        println!("{}", content);
                    }
                }
            }
            Err(e) => eprintln!("[cli] Error: {}", e),
        }
    }
}
