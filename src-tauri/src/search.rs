/// Web Search — multiple backend fallback (no API key needed)
pub async fn search(query: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .danger_accept_invalid_certs(true)
        .build().map_err(|e| format!("Client: {}", e))?;

    match ddg(&client, query).await { Ok(r) if !r.is_empty() => return Ok(r), _ => {} }
    match google_s(&client, query).await { Ok(r) if !r.is_empty() => return Ok(r), _ => {} }
    Err("All search backends failed".to_string())
}

/// DuckDuckGo Instant Answer (free, no API key)
async fn ddg(client: &reqwest::Client, q: &str) -> Result<String, String> {
    let data: serde_json::Value = client.get("https://api.duckduckgo.com/")
        .query(&[("q", q), ("format", "json"), ("no_html", "1")])
        .header("User-Agent", "Mozilla/5.0")
        .send().await.map_err(|e| format!("DDG: {}", e))?
        .json().await.map_err(|e| format!("DDG: {}", e))?;
    let mut r = Vec::new();
    if let Some(t) = data["AbstractText"].as_str() { if !t.is_empty() { r.push(format!("- {}", t)); } }
    if let Some(topics) = data["RelatedTopics"].as_array() {
        for t in topics.iter().take(5) {
            if let Some(text) = t["Text"].as_str() { r.push(format!("- {}", text)); }
        }
    }
    Ok(r.join("\n"))
}

/// Google fallback (HTML scrape)
async fn google_s(client: &reqwest::Client, q: &str) -> Result<String, String> {
    let enc: String = q.chars().map(|c| match c {
        'A'..='Z'|'a'..='z'|'0'..='9'|'-'|'_'|'.'|'~' => c.to_string(),
        ' ' => "+".to_string(),
        _ => format!("%{:02X}", c as u8),
    }).collect();
    let html = client.get(format!("https://www.google.com/search?q={}", enc))
        .header("User-Agent", "Mozilla/5.0")
        .send().await.map_err(|e| format!("G: {}", e))?
        .text().await.map_err(|e| format!("G: {}", e))?;
    let mut r = Vec::new();
    for line in html.lines() {
        if line.contains("<h3") {
            let c = line.split('<').filter(|s| !s.contains('>')).collect::<Vec<_>>().join(" ").trim().to_string();
            if c.len() > 15 { r.push(format!("- {}", c)); }
        }
    }
    Ok(r.iter().take(8).cloned().collect::<Vec<_>>().join("\n"))
}
