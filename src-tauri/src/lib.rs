use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;

mod memory;
mod personality;
mod hub;
mod skill_bundle;
mod desk;
mod sandbox;
mod knowledge;
mod subagent;
mod tool_layer;
pub mod cli;
mod search;
mod browser;
mod pty_manager;

use memory::MemoryStore;
use personality::default_personalities;
use hub::HubConfig;
use skill_bundle::InstalledSkill;
use sandbox::{PathGuard, PermissionLevel};

// Globals
static HUB_CONFIG: OnceLock<tokio::sync::Mutex<HubConfig>> = OnceLock::new();
fn hub_config() -> &'static tokio::sync::Mutex<HubConfig> { HUB_CONFIG.get_or_init(|| tokio::sync::Mutex::new(HubConfig::default())) }

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
fn http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build().expect("Failed to build HTTP client")
    })
}

fn memory_store() -> &'static MemoryStore { static S: OnceLock<MemoryStore> = OnceLock::new(); S.get_or_init(|| MemoryStore::new()) }
fn subagent_manager() -> &'static subagent::SubAgentManager { static M: OnceLock<subagent::SubAgentManager> = OnceLock::new(); M.get_or_init(|| subagent::SubAgentManager::new()) }
fn pty_manager() -> &'static pty_manager::PtyManager { static P: OnceLock<pty_manager::PtyManager> = OnceLock::new(); P.get_or_init(|| pty_manager::PtyManager::new()) }

fn cfg_dir() -> Result<PathBuf, String> { std::env::var("APPDATA").map(|a| PathBuf::from(a).join("ai-tools-hub")).map_err(|_| "no appdata".into()) }
fn load_hub_config() -> HubConfig { cfg_dir().ok().and_then(|d| std::fs::read_to_string(d.join("hub.json")).ok().and_then(|c| serde_json::from_str(&c).ok())).unwrap_or_default() }
fn save_hub_config(cfg: &HubConfig) { if let Ok(d) = cfg_dir() { if let Ok(j) = serde_json::to_string_pretty(cfg) { std::fs::write(d.join("hub.json"), j).ok(); } } }
fn desk_dir() -> PathBuf { let b = cfg_dir().unwrap_or_else(|_| PathBuf::from(".")); let d = b.join("desk"); std::fs::create_dir_all(&d).ok(); d }
fn vault_path() -> PathBuf {
    if let Ok(cfg) = hub_config().try_lock() { if !cfg.vault_path.is_empty() { return PathBuf::from(&cfg.vault_path); } }
    if let Ok(env) = std::env::var("AI_HUB_VAULT") { return PathBuf::from(env); }
    std::env::var("AI_HUB_VAULT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(r"D:\我的工作台"))
}
fn path_guard() -> &'static PathGuard { static P: OnceLock<PathGuard> = OnceLock::new(); P.get_or_init(|| { let mut pg = PathGuard::default(); pg.add_allowed(vault_path()); pg }) }

// Types
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMsg { pub role: String, pub content: String, #[serde(skip_serializing_if="Option::is_none")] pub toolId: Option<String> }
#[derive(Debug, Serialize)]
pub struct AiResponse { pub content: String, pub tokens: i32 }
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolConfig {
    pub id: String, pub name: String, pub icon: String, pub tool_type: String, pub enabled: bool,
    pub provider: String, pub api_base: String, pub model_name: String, pub api_key: String,
    pub command: String, pub args: Vec<String>, pub system_prompt: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Session { pub id: String, pub tool_id: String, pub title: String, pub messages: Vec<ChatMsg>, pub created: String, pub updated: String }

fn sessions_dir() -> Result<PathBuf, String> { let d = cfg_dir()?.join("sessions"); std::fs::create_dir_all(&d).ok(); Ok(d) }
fn load_tools() -> Vec<ToolConfig> { cfg_dir().ok().and_then(|d| std::fs::read_to_string(d.join("tools.json")).ok().and_then(|c| serde_json::from_str(&c).ok())).unwrap_or_default() }
fn cron_scheduler_inner() -> Arc<desk::CronScheduler> {
    static C: OnceLock<Arc<desk::CronScheduler>> = OnceLock::new();
    C.get_or_init(|| { let s = Arc::new(desk::CronScheduler::new()); tokio::spawn({ let c = s.clone(); async move { c.spawn().await } }); s }).clone()
}

// Vault indexing in background thread
fn index_vault_async() {
    std::thread::spawn(|| {
        let v = vault_path();
        if !v.exists() { return; }
        eprintln!("[kb] Starting index...");
        let mut count = 0usize;
        index_dir(&v, &v, &mut count);
        eprintln!("[kb] Indexed {} files total", count);
    });
}
fn index_dir(root: &Path, dir: &Path, count: &mut usize) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() { index_dir(root, &p, count); }
            else if p.extension().map(|e| e == "md").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy();
                    let id = format!("kb_{}", name);
                    if knowledge::get(&id).is_none() {
                        let _ = knowledge::save(&name, &content, "vault", &rel);
                        *count += 1;
                    }
                }
            }
        }
    }
}

// ── Streaming Chat ──
#[tauri::command]
async fn chat_completion_stream(
    app_handle: tauri::AppHandle,
    tool_id: String,
    messages: Vec<ChatMsg>,
    system_prompt: String,
) -> Result<AiResponse, String> {
    let tools = load_tools();
    let tool = tools.iter().find(|t| t.id == tool_id).ok_or("tool not found")?;

    // CLI tool via PTY
    if tool.tool_type == "process" {
        let msg = messages.last().ok_or("no message")?.content.clone();
        let mgr = pty_manager();
        if !mgr.list_sessions().contains(&tool_id) {
            let args: Vec<String> = tool.args.clone();
            mgr.start_session(&tool_id, &tool.command, &args).map_err(|e| format!("PTY: {}", e))?;
        }
        // Run blocking PTY chat in a dedicated thread via spawn_blocking
        let tid = tool_id.clone();
        let response = tokio::task::spawn_blocking(move || {
            mgr.chat(&tid, &msg)
        }).await.map_err(|e| format!("Task: {}", e))?.map_err(|e| format!("PTY: {}", e))?;

        app_handle.emit("stream-chunk", &response).unwrap_or(());
        app_handle.emit("stream-done", "").unwrap_or(());
        return Ok(AiResponse { content: response, tokens: 0 });
    }

    // API tool — validate tool_type
    if tool.tool_type != "api" {
        return Err(format!("Unknown tool_type '{}' for tool '{}'. Expected 'api' or 'process'.", tool.tool_type, tool.name));
    }
    if tool.api_key.is_empty() { return Err(format!("API Key not set: {}", tool.name)); }
    let sp = if system_prompt.is_empty() { "You are a helpful assistant.".into() } else { system_prompt };
    let last_msg = messages.last().map(|m| m.content.clone()).unwrap_or_default();
    let kb_ctx = knowledge::build_knowledge_context(&last_msg, 3);
    let sp = format!("{}{}", sp, kb_ctx);
    let mut msgs = vec![serde_json::json!({"role": "system", "content": sp})];
    for m in &messages { msgs.push(serde_json::json!({"role": m.role, "content": m.content})); }

    // ── Tool Calling Loop ──
    // Round 1: send with tools[] param
    //         → if LLM calls tools → execute → append results → next round
    // Round 2+: send messages only (results from prev round)
    //         → stream final text to frontend
    // Max 5 rounds to prevent infinite loops
    tool_layer::ensure_ready().await;
    let tool_defs = tool_layer::global_registry().list_defs().await;
    let has_tools = !tool_defs.is_empty();
    let max_rounds: usize = 5;
    let mut full = String::new();
    use futures::StreamExt;

    for round in 0..max_rounds {
        let mut body = serde_json::json!({
            "model": tool.model_name,
            "messages": msgs,
            "stream": true,
        });
        // Only send tool definitions on first round
        if has_tools && round == 0 {
            body["tools"] = serde_json::to_value(&tool_defs).unwrap_or_default();
        }

        let resp = http_client()
            .post(format!("{}/v1/chat/completions", tool.api_base))
            .header("Authorization", format!("Bearer {}", tool.api_key))
            .json(&body)
            .send().await.map_err(|e| format!("API: {}", e))?;

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut acc = tool_layer::ToolCallAccumulator::default();
        let mut round_content = String::new();
        let mut has_tool_calls = false;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Stream: {}", e))?;
            buf.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf = buf[pos + 1..].to_string();
                if line.starts_with("data: ") {
                    let d = &line[6..];
                    if d == "[DONE]" { break; }
                    else if let Ok(v) = serde_json::from_str::<serde_json::Value>(d) {
                        // Check for tool_calls delta
                        if has_tools && v["choices"][0]["delta"]["tool_calls"].is_array() {
                            has_tool_calls = true;
                            acc.feed(&v);
                        }
                        // Check for content delta
                        if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                            round_content.push_str(delta);
                            // Content is buffered during streaming.
                            // If this round ends with no tool calls, we flush below.
                            // If tool calls happen, content is thrown away (LLM re-synthesizes with tool results).
                        }
                    }
                }
            }
        }

        if has_tool_calls && acc.has_pending() {
            let calls = acc.collect();
            // Notify frontend about tool calls
            app_handle.emit("tool-call-start", &serde_json::json!({
                "tools": calls.iter().map(|c| serde_json::json!({
                    "name": c.name,
                    "args": c.arguments,
                })).collect::<Vec<_>>()
            })).unwrap_or(());

            // Store assistant message with tool_calls
            let tool_calls_json: Vec<serde_json::Value> = calls.iter().enumerate().map(|(i, c)| {
                serde_json::json!({
                    "id": format!("call_{}_{}", round, i),
                    "type": "function",
                    "function": {
                        "name": c.name,
                        "arguments": serde_json::to_string(&c.arguments).unwrap_or_default()
                    }
                })
            }).collect();
            msgs.push(serde_json::json!({
                "role": "assistant",
                "content": round_content,
                "tool_calls": tool_calls_json,
            }));

            // Execute tool calls and get results
            let results = tool_layer::execute_tool_calls(
                &tool_layer::global_registry(),
                &calls,
            ).await;
            for r in &results {
                msgs.push(r.clone());
            }

            // Notify frontend about tool results
            app_handle.emit("tool-call-end", &serde_json::json!({
                "results": results,
            })).unwrap_or(());

            continue; // Next round with tool results in context
        }

        // No tool calls — flush content to frontend
        if !round_content.is_empty() {
            full = round_content;
            app_handle.emit("stream-chunk", &full).unwrap_or(());
        }
        break;
    }

    app_handle.emit("stream-done", "").unwrap_or(());
    Ok(AiResponse { content: full, tokens: 0 })
}

// ── SubAgent ──
#[tauri::command]
async fn create_subagent(parent_id: String, tool_id: String, task: String) -> Result<String, String> {
    let id = subagent_manager().create(&parent_id, &tool_id, &task);
    let id2 = id.clone();
    let tid = tool_id.clone();
    let tk = task.clone();

    // Spawn a real task that calls the API to execute the sub-agent's task
    tokio::spawn(async move {
        let tools = load_tools();
        let tool = match tools.iter().find(|t| t.id == tid) {
            Some(t) => t.clone(),
            None => {
                subagent_manager().complete(&id2, "Tool not found", "error");
                return;
            }
        };

        if tool.tool_type != "api" || tool.api_key.is_empty() {
            subagent_manager().complete(&id2, "Only API tools are supported for sub-agents", "error");
            return;
        }

        let sp = format!("You are a focused sub-agent. Complete this task concisely:\n\n{}", tk);
        let resp = http_client()
            .post(format!("{}/v1/chat/completions", tool.api_base))
            .header("Authorization", format!("Bearer {}", tool.api_key))
            .json(&serde_json::json!({
                "model": tool.model_name,
                "messages": [
                    {"role": "system", "content": sp},
                    {"role": "user", "content": tk}
                ],
                "stream": false
            }))
            .send().await;

        match resp {
            Ok(r) => {
                if let Ok(data) = r.json::<serde_json::Value>().await {
                    let content = data["choices"][0]["message"]["content"]
                        .as_str().unwrap_or("(no response)").to_string();
                    subagent_manager().complete(&id2, &content, "done");
                } else {
                    subagent_manager().complete(&id2, "Failed to parse API response", "error");
                }
            }
            Err(e) => {
                subagent_manager().complete(&id2, &format!("API error: {}", e), "error");
            }
        }
    });

    Ok(id)
}
#[tauri::command]
async fn get_subagent_result(id: String) -> Result<Option<subagent::SubAgentSession>, String> { Ok(subagent_manager().get_result(&id)) }
#[tauri::command]
async fn list_subagents(parent_id: String) -> Result<Vec<subagent::SubAgentSession>, String> { Ok(subagent_manager().list_by_parent(&parent_id)) }

// ── Skill Learning ──
#[tauri::command]
async fn learn_skill_from_conversation(messages: Vec<ChatMsg>) -> Result<InstalledSkill, String> {
    let conv: Vec<String> = messages.iter().map(|m| format!("{}: {}", m.role, m.content)).collect();
    let text = conv.join("\n");
    let prompt = format!("Extract a reusable skill as JSON (name,description,prompt).\nConversation:\n{}\nJSON:", &text[..text.len().min(3000)]);
    let tools = load_tools();
    let tool = tools.iter().find(|t| t.tool_type == "api").ok_or("no API tool")?;
    let resp = http_client()
        .post(format!("{}/v1/chat/completions", tool.api_base))
        .header("Authorization", format!("Bearer {}", tool.api_key))
        .json(&serde_json::json!({"model": tool.model_name, "messages": [{"role":"user","content":prompt}], "stream": false}))
        .send().await.map_err(|e| e.to_string())?;
    let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let s = data["choices"][0]["message"]["content"].as_str().unwrap_or("{}");
    let v: serde_json::Value = serde_json::from_str(s).unwrap_or_default();
    let name = v["name"].as_str().unwrap_or("Learned");
    let desc = v["description"].as_str().unwrap_or("extracted");
    let prompt = v["prompt"].as_str().unwrap_or("");
    let installed = skill_bundle::install(&serde_json::json!({"name":format!("Learned: {}",name),"version":"1.0.0","description":desc,"author":"AI Hub","skills":[{"name":name,"description":desc,"prompt":prompt}]}).to_string())?;
    Ok(installed.into_iter().next().unwrap_or(InstalledSkill{id:"l".into(),bundle_name:"A".into(),name:name.into(),description:desc.into(),prompt:prompt.into(),version:"1.0.0".into(),enabled:true}))
}

// ── Commands ──
#[tauri::command] async fn get_tools() -> Result<Vec<ToolConfig>, String> { Ok(load_tools()) }
#[tauri::command] async fn save_tools(tools: Vec<ToolConfig>) -> Result<(), String> {
    let d = cfg_dir()?; std::fs::create_dir_all(&d).ok();
    std::fs::write(d.join("tools.json"), serde_json::to_string_pretty(&tools).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
#[tauri::command] async fn list_sessions() -> Result<Vec<Session>, String> {
    let dir = sessions_dir()?; let mut sessions: Vec<Session> = Vec::new();
    if let Ok(e) = std::fs::read_dir(dir) {
        for entry in e.flatten() {
            if let Ok(c) = std::fs::read_to_string(entry.path()) { if let Ok(s) = serde_json::from_str(&c) { sessions.push(s); } }
        }
    }
    sessions.sort_by(|a, b| b.updated.cmp(&a.updated));
    // Keep only last 50 sessions
    sessions.truncate(50);
    Ok(sessions)
}
#[tauri::command] async fn save_session(session: Session) -> Result<(), String> {
    let dir = sessions_dir()?;
    std::fs::write(dir.join(format!("{}.json", session.id)), serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let full = session.messages.iter().map(|m| format!("{}: {}", m.role, m.content)).collect::<Vec<_>>().join("\n");
    let short: String = full.chars().take(300).collect();
    let title = short.chars().take(80).collect::<String>();
    memory_store().save_summary(&session.id, &session.tool_id, &title, &short);
    Ok(())
}
#[tauri::command] async fn delete_session(session_id: String) -> Result<(), String> { std::fs::remove_file(sessions_dir()?.join(format!("{}.json", session_id))).map_err(|e| e.to_string()) }
#[tauri::command] async fn get_skills_list() -> Result<Vec<InstalledSkill>, String> { Ok(skill_bundle::list_installed()) }
#[tauri::command] async fn install_skill(p: String) -> Result<Vec<InstalledSkill>, String> { skill_bundle::install(&p) }
#[tauri::command] async fn uninstall_skill(n: String) -> Result<(), String> { skill_bundle::uninstall(&n) }
#[tauri::command] async fn search_knowledge(q: String, limit: Option<usize>) -> Result<Vec<knowledge::KnowledgeDoc>, String> { Ok(knowledge::search(&q, limit.unwrap_or(10))) }
#[tauri::command] async fn save_knowledge(t: String, c: String, g: String, s: String) -> Result<knowledge::KnowledgeDoc, String> { knowledge::save(&t, &c, &g, &s) }
#[tauri::command] async fn list_knowledge_topics() -> Result<Vec<knowledge::Topic>, String> { Ok(knowledge::list_topics()) }
#[tauri::command] async fn delete_knowledge(id: String) -> Result<(), String> { knowledge::delete(&id) }
#[tauri::command] async fn get_knowledge_doc(id: String) -> Result<knowledge::KnowledgeDoc, String> { knowledge::get(&id).ok_or_else(|| "Not found".into()) }
#[tauri::command] async fn web_search(q: String) -> Result<String, String> { Ok(format!("[Search]\n---\n{}\n---", search::search(&q).await?)) }
#[tauri::command] async fn browser_search(q: String) -> Result<String, String> {
    match browser::search(&q, 9222).await {
        Ok(r) => Ok(format!("[Browser]\n---\n{}\n---", r)),
        Err(e) => { eprintln!("[browser] {} fallback DDG", e); web_search(q).await }
    }
}
#[tauri::command] async fn set_auto_start(enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")] {
        if let Some(p) = std::env::var("APPDATA").ok().and_then(|a| PathBuf::from(&a).parent().map(|p| p.join("Microsoft\\Windows\\Start Menu\\Programs\\Startup\\AI Tools Hub.lnk"))) {
            if enable {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                Command::new("powershell").args(["-Command", &format!("$ws=New-Object -ComObject WScript.Shell;$s=$ws.CreateShortcut('{}');$s.TargetPath='{}';$s.Save()", p.display(), exe.display())]).output().map_err(|e| e.to_string())?;
            } else { std::fs::remove_file(p).ok(); }
        }
    }
    Ok(())
}
#[tauri::command] async fn check_auto_start() -> Result<bool, String> {
    #[cfg(target_os = "windows")] { Ok(std::env::var("APPDATA").ok().and_then(|a| PathBuf::from(&a).parent().map(|p| p.join("Microsoft\\Windows\\Start Menu\\Programs\\Startup\\AI Tools Hub.lnk"))).map(|p| p.exists()).unwrap_or(false)) }
    #[cfg(not(target_os = "windows"))] Ok(false)
}
#[tauri::command] async fn check_path_access(path: String, level: String) -> Result<serde_json::Value, String> {
    let perm = match level.as_str() { "full" => PermissionLevel::FullAccess, _ => PermissionLevel::Restricted };
    let result = path_guard().check(Path::new(&path), &perm);
    Ok(serde_json::json!({"path": path, "allowed": result.is_ok(), "message": result.err().unwrap_or_default()}))
}
#[tauri::command] async fn get_sandbox_status() -> Result<serde_json::Value, String> { Ok(serde_json::json!({"active":true,"allowed_dirs":path_guard().allowed_dirs_debug()})) }
#[tauri::command] async fn get_cron_jobs() -> Result<Vec<desk::CronJob>, String> { Ok(cron_scheduler_inner().list().await) }
#[tauri::command] async fn add_cron_job(j: desk::CronJob) -> Result<(), String> { cron_scheduler_inner().add(j).await; Ok(()) }
#[tauri::command] async fn remove_cron_job(id: String) -> Result<(), String> { cron_scheduler_inner().remove(&id).await; Ok(()) }
#[tauri::command] async fn get_desk_notes() -> Result<Vec<desk::DeskNote>, String> { Ok(desk::list_desk_notes(&desk_dir())) }
#[tauri::command] async fn write_desk_note(f: String, c: String) -> Result<(), String> { if f.contains("..") || f.contains("/") || f.contains("\\") { return Err("Invalid filename".into()); } std::fs::write(desk_dir().join(&f), &c).map_err(|e| e.to_string()) }
#[tauri::command] async fn get_hub_config() -> Result<HubConfig, String> { let c = hub_config().lock().await; Ok(c.clone()) }
#[tauri::command] async fn set_hub_config(cfg: HubConfig) -> Result<(), String> { let mut c = hub_config().lock().await; *c = cfg.clone(); save_hub_config(&cfg); Ok(()) }
#[tauri::command] async fn get_hub_url() -> Result<String, String> { Ok("http://localhost:27125".into()) }
#[tauri::command] async fn get_personalities() -> Result<Vec<personality::PersonalityTemplate>, String> { Ok(default_personalities()) }
#[derive(Debug, Serialize)] pub struct SearchResult { pub content: String, pub source: String, pub timestamp: String }
#[tauri::command] async fn save_memory(t: String, m: String, c: String, k: String) -> Result<i64, String> { Ok(memory_store().save(&t, None, &m, &c, &k, "")) }
#[tauri::command] async fn search_memory(q: String, tool_id: Option<String>) -> Result<Vec<SearchResult>, String> {
    let t = tool_id.as_deref().filter(|s| !s.is_empty());
    let mut r = Vec::new();
    for m in memory_store().search(&q, t, 10) { r.push(SearchResult{content:m.content,source:m.tool_id,timestamp:m.created_at}); }
    for s in memory_store().search_summaries(&q, t, 5) { r.push(SearchResult{content:format!("[Conv] {}: {}",s.title,s.summary),source:s.tool_id,timestamp:s.updated_at}); }
    Ok(r)
}
#[tauri::command] async fn get_recent_memories(t: String) -> Result<Vec<memory::Memory>, String> { Ok(memory_store().get_recent(&t, 20)) }
#[tauri::command] async fn delete_memory(id: i64) -> Result<bool, String> { Ok(memory_store().delete(id)) }
#[derive(Debug, Serialize)] pub struct ObsidianFile { pub path: String, pub is_dir: bool }
#[tauri::command] async fn obsidian_list_dir(sub: String) -> Result<Vec<ObsidianFile>, String> {
    let root = vault_path(); let full = root.join(&sub); path_guard().check_read(&full)?;
    let mut files = Vec::new();
    if let Ok(e) = std::fs::read_dir(&full) { for entry in e.flatten() { let p = entry.path(); files.push(ObsidianFile{path:p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().replace('\\',"/"),is_dir:p.is_dir()}); } }
    Ok(files)
}
#[tauri::command] async fn obsidian_read_fs(path: String) -> Result<String, String> { let full = vault_path().join(&path); path_guard().check_read(&full)?; std::fs::read_to_string(&full).map_err(|e| format!("Read: {}", e)) }
#[tauri::command] async fn obsidian_write_fs(p: String, c: String) -> Result<(), String> {
    let full = vault_path().join(&p); path_guard().check_read(&full)?;
    if let Some(parent) = full.parent() { std::fs::create_dir_all(parent).ok(); } std::fs::write(&full, &c).map_err(|e| e.to_string())
}

// ── PTY Test ──
static PTY_TEST: OnceLock<std::sync::Mutex<pty_manager::PtyManager>> = OnceLock::new();
fn pty_test() -> &'static std::sync::Mutex<pty_manager::PtyManager> { PTY_TEST.get_or_init(|| std::sync::Mutex::new(pty_manager::PtyManager::new())) }

#[tauri::command]
async fn start_pty_test(command: String, args: Vec<String>) -> Result<(), String> {
    let mgr = pty_test(); let m = mgr.lock().map_err(|e| e.to_string())?;
    m.start_session("pty_test", &command, &args).map_err(|e| format!("PTY: {}", e))
}

#[tauri::command]
async fn pty_test_send(input: String) -> Result<String, String> {
    let mgr = pty_test(); let m = mgr.lock().map_err(|e| e.to_string())?;
    m.chat("pty_test", &input)
}

#[tauri::command]
async fn pty_test_stop() -> Result<(), String> {
    let mgr = pty_test(); let m = mgr.lock().map_err(|e| e.to_string())?;
    m.close_session("pty_test"); Ok(())
}

#[tauri::command]
async fn save_pty_tool(name: String, icon: String, command: String, args: Vec<String>) -> Result<(), String> {
    let mut tools = load_tools(); let id = format!("tool_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
    tools.push(ToolConfig { id, name, icon, tool_type: "process".into(), enabled: true, provider: String::new(), api_base: String::new(), model_name: String::new(), api_key: String::new(), command, args, system_prompt: String::new() });
    let d = cfg_dir()?; std::fs::write(d.join("tools.json"), serde_json::to_string_pretty(&tools).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
static CONTEXT_BUFFER: OnceLock<std::sync::Mutex<Vec<String>>> = OnceLock::new();
fn context_buffer() -> &'static std::sync::Mutex<Vec<String>> { CONTEXT_BUFFER.get_or_init(|| std::sync::Mutex::new(Vec::new())) }
#[tauri::command] async fn push_context(c: String) -> Result<(), String> { let mut b = context_buffer().lock().map_err(|e| e.to_string())?; b.push(c); if b.len() > 20 { b.remove(0); } Ok(()) }
#[tauri::command] async fn pop_context() -> Result<Option<String>, String> { Ok(context_buffer().lock().map_err(|e| e.to_string())?.pop()) }
#[tauri::command] async fn peek_context() -> Result<Vec<String>, String> { Ok(context_buffer().lock().map_err(|e| e.to_string())?.clone()) }
#[tauri::command] async fn clear_context() -> Result<(), String> { context_buffer().lock().map_err(|e| e.to_string())?.clear(); Ok(()) }

// ── Main ──
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let saved = load_hub_config();
    if let Ok(mut c) = hub_config().try_lock() { *c = saved; }
    // Initialize tool calling layer
    tauri::async_runtime::spawn(async {
        tool_layer::init_builtin_tools().await;
    });

    tauri::Builder::default()
        .setup(|_app| {
            index_vault_async(); // background thread, doesn't block UI
            let _ = cron_scheduler_inner();
            let watch = desk_dir();
            std::thread::spawn(move || desk::start_watcher(watch, |e| { eprintln!("[desk] {:?} {}", e.kind, e.path); }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_tools, save_tools, list_sessions, save_session, delete_session,
            get_skills_list, install_skill, uninstall_skill,
            search_knowledge, save_knowledge, list_knowledge_topics, delete_knowledge, get_knowledge_doc,
            web_search, browser_search, set_auto_start, check_auto_start,
            get_personalities,
            save_memory, search_memory, get_recent_memories, delete_memory,
            obsidian_list_dir, obsidian_read_fs, obsidian_write_fs,
            push_context, pop_context, peek_context, clear_context,
            get_cron_jobs, add_cron_job, remove_cron_job,
            get_desk_notes, write_desk_note,
            check_path_access, get_sandbox_status,
            chat_completion_stream,
            start_pty_test, pty_test_send, pty_test_stop, save_pty_tool,
            create_subagent, get_subagent_result, list_subagents,
            learn_skill_from_conversation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
