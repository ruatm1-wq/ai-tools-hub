/// Tool Calling Layer — 工具注册中心 + 执行器
///
/// 让 AI 能主动调用工具（读文件、搜索、记忆等），
/// 类似 OpenAI Function Calling / Claude Artifacts。
///
/// 架构：
///   ToolDef (定义) → ToolRegistry (注册表) → execute() (执行)
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

// ── Tool Definition ──

/// 工具定义 — 发送给 LLM 的 JSON Schema 描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    /// 工具名称，如 "read_file"
    pub name: String,
    /// 工具描述，LLM 据此决定是否调用
    pub description: String,
    /// 参数 JSON Schema（OpenAI function calling 格式）
    pub parameters: Value,
}

// ── Tool Result ──

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub value: Value,
    pub error: Option<String>,
}

impl ToolResult {
    pub fn ok(value: impl Into<Value>) -> Self {
        ToolResult { success: true, value: value.into(), error: None }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        ToolResult { success: false, value: Value::Null, error: Some(msg.into()) }
    }
}

// ── Handler Type ──

/// 工具 handler 类型：接收 JSON params，返回 ToolResult
type HandlerFn = Arc<dyn Fn(Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> + Send + Sync>;

/// 辅助函数：从 async fn 创建 HandlerFn
pub fn make_handler<F, Fut>(f: F) -> HandlerFn
where
    F: Fn(Value) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ToolResult> + Send + 'static,
{
    Arc::new(move |params| Box::pin(f(params)))
}

// ── Tool Registry ──

/// 工具注册中心 — 线程安全，全局唯一
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, ToolDef>>,
    handlers: RwLock<HashMap<String, HandlerFn>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry {
            tools: RwLock::new(HashMap::new()),
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// 注册一个工具
    pub async fn register(&self, def: ToolDef, handler: HandlerFn) {
        let name = def.name.clone();
        self.tools.write().await.insert(name.clone(), def);
        self.handlers.write().await.insert(name, handler);
    }

    /// 按名称获取工具定义
    pub async fn get_def(&self, name: &str) -> Option<ToolDef> {
        self.tools.read().await.get(name).cloned()
    }

    /// 列出所有工具定义（用于发送给 LLM）
    pub async fn list_defs(&self) -> Vec<ToolDef> {
        self.tools.read().await.values().cloned().collect()
    }

    /// 执行一个工具调用
    pub async fn execute(&self, name: &str, params: Value) -> ToolResult {
        match self.handlers.read().await.get(name) {
            Some(handler) => handler(params).await,
            None => ToolResult::err(format!("Unknown tool: {}", name)),
        }
    }
}

// ── Global Registry ──

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static GLOBAL_REGISTRY: OnceLock<Arc<ToolRegistry>> = OnceLock::new();
static TOOLS_READY: AtomicBool = AtomicBool::new(false);

/// 获取全局 ToolRegistry（懒初始化）
pub fn global_registry() -> Arc<ToolRegistry> {
    GLOBAL_REGISTRY
        .get_or_init(|| Arc::new(ToolRegistry::new()))
        .clone()
}

/// 等待工具初始化完成
pub async fn ensure_ready() {
    while !TOOLS_READY.load(Ordering::Acquire) {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

/// 标记初始化完成（在 init_builtin_tools 末尾调用）
pub fn mark_ready() {
    TOOLS_READY.store(true, Ordering::Release);
}

// ═══════════════════════════════════════════════════════════
//  Built-in Tool Handlers
// ═══════════════════════════════════════════════════════════

/// read_file(path) — 从 vault 读取文件
async fn handle_read_file(params: Value) -> ToolResult {
    let path = params["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolResult::err("Missing 'path' parameter");
    }
    let vault = crate::vault_path();
    let full = vault.join(path);
    if let Err(e) = crate::path_guard().check_read(&full) {
        return ToolResult::err(format!("Path not allowed: {}", e));
    }
    match std::fs::read_to_string(&full) {
        Ok(content) => ToolResult::ok(serde_json::json!({
            "path": path,
            "content": content,
            "size": content.len()
        })),
        Err(e) => ToolResult::err(format!("Read failed: {}", e)),
    }
}

/// write_file(path, content) — 写入文件到 vault
async fn handle_write_file(params: Value) -> ToolResult {
    let path = params["path"].as_str().unwrap_or("");
    let content = params["content"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolResult::err("Missing 'path' parameter");
    }
    let vault = crate::vault_path();
    let full = vault.join(path);
    if let Err(e) = crate::path_guard().check_read(&full) {
        return ToolResult::err(format!("Path not allowed: {}", e));
    }
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    match std::fs::write(&full, content) {
        Ok(()) => ToolResult::ok(serde_json::json!({"path": path, "written": content.len()})),
        Err(e) => ToolResult::err(format!("Write failed: {}", e)),
    }
}

/// search_knowledge(query, limit?) — 搜索知识库索引
async fn handle_search_knowledge(params: Value) -> ToolResult {
    let query = params["query"].as_str().unwrap_or("");
    let limit = params["limit"].as_u64().unwrap_or(5) as usize;
    if query.is_empty() {
        return ToolResult::err("Missing 'query' parameter");
    }
    let results = crate::knowledge::search(query, limit);
    ToolResult::ok(serde_json::json!({
        "query": query,
        "results": results.iter().map(|d| serde_json::json!({
            "title": d.title,
            "content": d.content.chars().take(500).collect::<String>(),
            "source": d.source,
        })).collect::<Vec<_>>()
    }))
}

/// web_search(query) — 联网搜索
async fn handle_web_search(params: Value) -> ToolResult {
    let query = params["query"].as_str().unwrap_or("");
    if query.is_empty() {
        return ToolResult::err("Missing 'query' parameter");
    }
    match crate::browser::search(query, 9222).await {
        Ok(r) => ToolResult::ok(serde_json::json!({"query": query, "results": r})),
        Err(_) => match crate::search::search(query).await {
            Ok(r) => ToolResult::ok(serde_json::json!({"query": query, "results": r})),
            Err(e) => ToolResult::err(format!("Web search failed: {}", e)),
        },
    }
}

/// memory_search(query) — 搜索历史记忆
async fn handle_memory_search(params: Value) -> ToolResult {
    let query = params["query"].as_str().unwrap_or("");
    if query.is_empty() {
        return ToolResult::err("Missing 'query' parameter");
    }
    let store = crate::memory_store();
    let results = store.search(query, None, 10);
    ToolResult::ok(serde_json::json!({
        "query": query,
        "results": results.iter().map(|m| serde_json::json!({
            "content": m.content.chars().take(300).collect::<String>(),
            "type": m.memory_type,
            "created": m.created_at,
        })).collect::<Vec<_>>()
    }))
}

/// memory_save(content, keywords) — 保存重要信息到记忆
async fn handle_memory_save(params: Value) -> ToolResult {
    let content = params["content"].as_str().unwrap_or("");
    let keywords = params["keywords"].as_str().unwrap_or("");
    if content.is_empty() {
        return ToolResult::err("Missing 'content' parameter");
    }
    let store = crate::memory_store();
    let id = store.save("tool", None, "fact", content, keywords, "");
    ToolResult::ok(serde_json::json!({"id": id, "saved": true}))
}

/// run_code(code, language?) — 执行代码片段
async fn handle_run_code(params: Value) -> ToolResult {
    let code = params["code"].as_str().unwrap_or("");
    let language = params["language"].as_str().unwrap_or("python");
    if code.is_empty() {
        return ToolResult::err("Missing 'code' parameter");
    }

    let (interpreter, ext) = match language {
        "python" => ("python", ".py"),
        "javascript" => ("node", ".js"),
        "shell" => ("cmd.exe", ".bat"),
        _ => return ToolResult::err(format!("Unsupported language: {}", language)),
    };

    let tmp_dir = std::env::temp_dir().join("ai-tools-hub-codes");
    std::fs::create_dir_all(&tmp_dir).ok();
    let tmp_file = tmp_dir.join(format!("code_{}", ext));
    if let Err(e) = std::fs::write(&tmp_file, code) {
        return ToolResult::err(format!("Write temp file failed: {}", e));
    }

    use std::process::Command;
    let output = if language == "shell" {
        Command::new(interpreter).args(["/c", tmp_file.to_str().unwrap_or("")]).output()
    } else {
        Command::new(interpreter).arg(tmp_file.to_str().unwrap_or("")).output()
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            std::fs::remove_file(&tmp_file).ok();
            ToolResult::ok(serde_json::json!({
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": out.status.code().unwrap_or(-1),
            }))
        }
        Err(e) => ToolResult::err(format!("Execution failed: {}", e)),
    }
}

// ═══════════════════════════════════════════════════════════
//  Init — 注册所有内置工具
// ═══════════════════════════════════════════════════════════

/// 初始化并注册所有内置工具
pub async fn init_builtin_tools() {
    let registry = global_registry();

    registry.register(
        ToolDef {
            name: "read_file".into(),
            description: "Read a file from the vault/knowledge base. Returns the file content as text.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path relative to vault root (e.g. 'notes/meeting.md')"
                    }
                },
                "required": ["path"]
            }),
        },
        make_handler(handle_read_file),
    ).await;

    registry.register(
        ToolDef {
            name: "write_file".into(),
            description: "Write content to a file in the vault. Creates parent directories if needed.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path relative to vault root (e.g. 'notes/output.md')"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write"
                    }
                },
                "required": ["path", "content"]
            }),
        },
        make_handler(handle_write_file),
    ).await;

    registry.register(
        ToolDef {
            name: "search_knowledge".into(),
            description: "Search the knowledge base (indexed markdown files) for relevant documents.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search keywords"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results (default 5)",
                        "default": 5
                    }
                },
                "required": ["query"]
            }),
        },
        make_handler(handle_search_knowledge),
    ).await;

    registry.register(
        ToolDef {
            name: "web_search".into(),
            description: "Search the web. Uses Chrome CDP first, falls back to DuckDuckGo/Google.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                },
                "required": ["query"]
            }),
        },
        make_handler(handle_web_search),
    ).await;

    registry.register(
        ToolDef {
            name: "memory_search".into(),
            description: "Search past conversation memories for relevant facts from history.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "What to search for in memories"
                    }
                },
                "required": ["query"]
            }),
        },
        make_handler(handle_memory_search),
    ).await;

    registry.register(
        ToolDef {
            name: "memory_save".into(),
            description: "Save an important fact to memory for future reference.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The fact/content to remember"
                    },
                    "keywords": {
                        "type": "string",
                        "description": "Comma-separated keywords for search"
                    }
                },
                "required": ["content", "keywords"]
            }),
        },
        make_handler(handle_memory_save),
    ).await;

    registry.register(
        ToolDef {
            name: "run_code".into(),
            description: "Execute a piece of code (Python/JavaScript/shell) and return the output.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "The code to execute"
                    },
                    "language": {
                        "type": "string",
                        "description": "python / javascript / shell",
                        "enum": ["python", "javascript", "shell"],
                        "default": "python"
                    }
                },
                "required": ["code"]
            }),
        },
        make_handler(handle_run_code),
    ).await;

    mark_ready();
}

// ═══════════════════════════════════════════════════════════
//  Tool Executor — 在 LLM 流中解析 tool_calls 并执行
// ═══════════════════════════════════════════════════════════

/// 解析 stream delta 中的 tool_calls，收集完整参数
///
/// OpenAI 流式 tool_calls 格式：
/// ```json
/// {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"read_file","arguments":"{\"path\":\""}}]}}]}
/// {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"notes.md\"}"}}]}}]}
/// ```
/// index 用于去重，arguments 是流式拼接的 JSON 字符串
#[derive(Debug, Clone, Default)]
pub struct ToolCallAccumulator {
    /// index → (name, arguments_partial)
    calls: HashMap<usize, (String, String)>,
}

impl ToolCallAccumulator {
    /// 处理一个 delta chunk，返回是否还有未完成的 tool_call
    pub fn feed(&mut self, chunk: &Value) -> bool {
        if let Some(tool_calls) = chunk["choices"][0]["delta"]["tool_calls"].as_array() {
            for tc in tool_calls {
                let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                let name = tc["function"]["name"].as_str().unwrap_or("");
                let args = tc["function"]["arguments"].as_str().unwrap_or("");

                let entry = self.calls.entry(idx).or_insert_with(|| (String::new(), String::new()));
                if !name.is_empty() {
                    entry.0.push_str(name);
                }
                if !args.is_empty() {
                    entry.1.push_str(args);
                }
            }
            return true;
        }
        false
    }

    /// 收集所有完整的 tool_call 并清空
    pub fn collect(&mut self) -> Vec<ToolCall> {
        let mut result = Vec::new();
        for (idx, (name, args_str)) in self.calls.drain() {
            let args: Value = serde_json::from_str(&args_str).unwrap_or(Value::Null);
            result.push(ToolCall {
                index: idx,
                name,
                arguments: args,
            });
        }
        result
    }

    /// 是否有正在收集的 tool_calls
    pub fn has_pending(&self) -> bool {
        !self.calls.is_empty()
    }
}

/// 一个完整的工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub index: usize,
    pub name: String,
    pub arguments: Value,
}

/// 执行一批 tool_calls 并返回结果消息（用于追加到 messages 数组）
pub async fn execute_tool_calls(
    registry: &ToolRegistry,
    calls: &[ToolCall],
) -> Vec<Value> {
    let mut results = Vec::new();
    for call in calls {
        let result = registry.execute(&call.name, call.arguments.clone()).await;
        results.push(serde_json::json!({
            "role": "tool",
            "tool_call_id": format!("call_{}", call.index),
            "name": call.name,
            "content": serde_json::to_string(&result.value).unwrap_or_default(),
        }));
    }
    results
}

// ═══════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_register_and_lookup() {
        let registry = ToolRegistry::new();

        registry.register(
            ToolDef {
                name: "test_tool".into(),
                description: "A test tool".into(),
                parameters: serde_json::json!({"type": "object", "properties": {}}),
            },
            make_handler(|_: Value| async { ToolResult::ok(serde_json::json!({"ok": true})) }),
        ).await;

        let def = registry.get_def("test_tool").await;
        assert!(def.is_some());
        assert_eq!(def.unwrap().name, "test_tool");

        let unknown = registry.get_def("nonexistent").await;
        assert!(unknown.is_none());
    }

    #[tokio::test]
    async fn test_list_defs() {
        let registry = ToolRegistry::new();
        registry.register(
            ToolDef {
                name: "tool_a".into(),
                description: "A".into(),
                parameters: serde_json::json!({}),
            },
            make_handler(|_: Value| async { ToolResult::ok(serde_json::json!({"ok": true})) }),
        ).await;
        registry.register(
            ToolDef {
                name: "tool_b".into(),
                description: "B".into(),
                parameters: serde_json::json!({}),
            },
            make_handler(|_: Value| async { ToolResult::ok(serde_json::json!({"ok": true})) }),
        ).await;

        let defs = registry.list_defs().await;
        assert_eq!(defs.len(), 2);
    }

    #[tokio::test]
    async fn test_execute_success() {
        let registry = ToolRegistry::new();
        registry.register(
            ToolDef {
                name: "echo".into(),
                description: "Echo".into(),
                parameters: serde_json::json!({"type":"object","properties":{"msg":{"type":"string"}}}),
            },
            make_handler(|params: Value| async move {
                ToolResult::ok(params)
            }),
        ).await;

        let result = registry.execute("echo", serde_json::json!({"msg": "hello"})).await;
        assert!(result.success);
        assert_eq!(result.value["msg"], "hello");
    }

    #[tokio::test]
    async fn test_execute_unknown_tool() {
        let registry = ToolRegistry::new();
        let result = registry.execute("no_such_tool", serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_tool_call_accumulator() {
        let mut acc = ToolCallAccumulator::default();

        // Simulate streaming delta
        let chunk1 = serde_json::json!({
            "choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"name": "read_file", "arguments": "{\"path\":\""}}]}}]
        });
        let chunk2 = serde_json::json!({
            "choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": "config.json\"}"}}]}}]
        });

        assert!(acc.feed(&chunk1));
        assert!(acc.feed(&chunk2));
        assert!(acc.has_pending());

        let calls = acc.collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "config.json");
        assert!(!acc.has_pending());
    }
}
