/// SubAgent Manager — create and manage independent agent sessions
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentSession {
    pub id: String,
    pub parent_id: String,     // who created this sub-agent
    pub tool_id: String,       // which tool to use
    pub task: String,          // the task description
    pub status: String,        // "running" | "done" | "error"
    pub result: String,
    pub created_at: String,
    pub completed_at: String,
}

pub struct SubAgentManager {
    sessions: Mutex<HashMap<String, SubAgentSession>>,
}

impl SubAgentManager {
    pub fn new() -> Self {
        SubAgentManager { sessions: Mutex::new(HashMap::new()) }
    }

    /// Create a sub-agent session and start processing
    pub fn create(&self, parent_id: &str, tool_id: &str, task: &str) -> String {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let id = format!("sub_{}_{}", now, parent_id.chars().take(4).collect::<String>());

        let session = SubAgentSession {
            id: id.clone(),
            parent_id: parent_id.to_string(),
            tool_id: tool_id.to_string(),
            task: task.to_string(),
            status: "running".into(),
            result: String::new(),
            created_at: now.to_string(),
            completed_at: String::new(),
        };

        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(id.clone(), session);
        id
    }

    /// Set the result for a sub-agent session
    pub fn complete(&self, id: &str, result: &str, status: &str) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs().to_string()).unwrap_or_default();
        if let Some(session) = self.sessions.lock().unwrap().get_mut(id) {
            session.result = result.to_string();
            session.status = status.to_string();
            session.completed_at = now;
        }
    }

    /// Get the result for a sub-agent session
    pub fn get_result(&self, id: &str) -> Option<SubAgentSession> {
        self.sessions.lock().unwrap().get(id).cloned()
    }

    /// List sessions by parent
    pub fn list_by_parent(&self, parent_id: &str) -> Vec<SubAgentSession> {
        self.sessions.lock().unwrap().values()
            .filter(|s| s.parent_id == parent_id)
            .cloned()
            .collect()
    }
}
