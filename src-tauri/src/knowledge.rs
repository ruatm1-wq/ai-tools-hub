/// Shared Knowledge Base — structured markdown documents + full-text search
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KnowledgeDoc {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KnowledgeIndex {
    pub topics: Vec<Topic>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Topic {
    pub name: String,
    pub description: String,
    pub doc_count: usize,
}

fn kb_dir() -> Result<PathBuf, String> {
    let base = std::env::var("APPDATA")
        .map(|a| PathBuf::from(a).join("ai-tools-hub"))
        .map_err(|_| String::from("no appdata"))?;
    let dir = base.join("knowledge");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn doc_path(id: &str) -> Result<PathBuf, String> {
    Ok(kb_dir()?.join(format!("{}.json", id)))
}

pub fn save(title: &str, content: &str, tags: &str, source: &str) -> Result<KnowledgeDoc, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs().to_string()).unwrap_or_default();
    let id = title.trim().to_lowercase()
        .replace(' ', "_")
        .replace(|c: char| !c.is_alphanumeric() && c != '_', "")
        + "_" + &now;

    let doc = KnowledgeDoc {
        id: id.clone(), title: title.to_string(),
        content: content.to_string(), tags: tags.to_string(),
        source: source.to_string(), created_at: now.clone(), updated_at: now,
    };
    let json = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(doc_path(&id)?, &json).map_err(|e| format!("Write failed: {}", e))?;
    Ok(doc)
}

pub fn search(query: &str, limit: usize) -> Vec<KnowledgeDoc> {
    let dir = match kb_dir() { Ok(d) => d, Err(_) => return Vec::new() };
    let pattern = query.to_lowercase();
    let mut results = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(doc) = serde_json::from_str::<KnowledgeDoc>(&content) {
                        let search_text = format!("{} {} {} {}", doc.title, doc.content, doc.tags, doc.source).to_lowercase();
                        if pattern.is_empty() || search_text.contains(&pattern) {
                            results.push(doc);
                        }
                    }
                }
            }
        }
    }
    results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    results.truncate(limit);
    results
}

pub fn list_topics() -> Vec<Topic> {
    let docs = search("", 1000);
    let mut topic_map: std::collections::HashMap<String, (String, usize)> = std::collections::HashMap::new();
    for doc in &docs {
        for tag in doc.tags.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()) {
            let entry = topic_map.entry(tag.to_string()).or_insert((format!("Docs tagged '{}'", tag), 0));
            entry.1 += 1;
        }
    }
    topic_map.into_iter()
        .map(|(n, (d, c))| Topic { name: n, description: d, doc_count: c })
        .collect()
}

pub fn delete(id: &str) -> Result<(), String> {
    let path = doc_path(id)?;
    if path.exists() { std::fs::remove_file(&path).map_err(|e| format!("Delete failed: {}", e)) }
    else { Err(format!("Document '{}' not found", id)) }
}

pub fn get(id: &str) -> Option<KnowledgeDoc> {
    doc_path(id).ok().and_then(|p| std::fs::read_to_string(&p).ok())
        .and_then(|c| serde_json::from_str(&c).ok())
}

/// Build context summary for injection into agent prompts
pub fn build_knowledge_context(query: &str, max_docs: usize) -> String {
    let docs = search(query, max_docs);
    if docs.is_empty() { return String::new(); }
    let mut ctx = String::from("\n\n[Shared Knowledge Base — relevant documents]\n");
    for (i, doc) in docs.iter().enumerate() {
        let preview = if doc.content.len() > 300 { format!("{}...", &doc.content[..300]) } else { doc.content.clone() };
        ctx.push_str(&format!("--- Doc {}: {} (tags: {}) ---\n{}\n\n", i + 1, doc.title, doc.tags, preview));
    }
    ctx.push_str("[/Shared Knowledge Base]");
    ctx
}
