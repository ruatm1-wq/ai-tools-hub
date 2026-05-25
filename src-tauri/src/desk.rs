/// Desk: Cron scheduler + File watcher + Note (笺) system.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub interval_secs: u64,
    pub action: String,
    pub command: String,
    pub enabled: bool,
    pub last_run: String,
    pub next_run: String,
}

impl CronJob {
    pub fn new(id: &str, name: &str, interval_secs: u64, command: &str) -> Self {
        CronJob {
            id: id.to_string(),
            name: name.to_string(),
            interval_secs,
            action: "shell".into(),
            command: command.to_string(),
            enabled: true,
            last_run: String::new(),
            next_run: String::new(),
        }
    }
}

pub struct CronScheduler {
    jobs: Arc<Mutex<Vec<CronJob>>>,
}

impl CronScheduler {
    pub fn new() -> Self {
        CronScheduler { jobs: Arc::new(Mutex::new(Vec::new())) }
    }

    pub fn get_jobs(&self) -> Arc<Mutex<Vec<CronJob>>> {
        self.jobs.clone()
    }

    pub async fn add(&self, job: CronJob) {
        self.jobs.lock().await.push(job);
    }

    pub async fn remove(&self, id: &str) {
        self.jobs.lock().await.retain(|j| j.id != id);
    }

    pub async fn list(&self) -> Vec<CronJob> {
        self.jobs.lock().await.clone()
    }

    pub async fn spawn(self: Arc<Self>) {
        let last_runs: Arc<std::sync::Mutex<std::collections::HashMap<String, u64>>> =
            Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));

        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let jobs = self.jobs.lock().await;
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

            let to_run: Vec<CronJob> = jobs.iter()
                .filter(|j| j.enabled)
                .filter(|j| {
                    let last = last_runs.lock().unwrap().get(&j.id).copied().unwrap_or(0);
                    now - last >= j.interval_secs
                })
                .cloned()
                .collect();

            for j in &to_run {
                last_runs.lock().unwrap().insert(j.id.clone(), now);
            }
            std::mem::drop(jobs);

            for j in to_run {
                tokio::spawn(async move { run_job(j).await; });
            }
        }
    }
}

async fn run_job(job: CronJob) {
    eprintln!("[desk] Cron: {} | {}", job.name, job.command);

    let output = if cfg!(target_os = "windows") {
        tokio::process::Command::new("cmd")
            .args(["/C", &job.command])
            .output().await
    } else {
        tokio::process::Command::new("sh")
            .args(["-c", &job.command])
            .output().await
    };

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            if !stdout.is_empty() { eprintln!("[desk] Output: {}", stdout.trim()); }
            if !stderr.is_empty() { eprintln!("[desk] Error: {}", stderr.trim()); }
        }
        Err(e) => eprintln!("[desk] Cron failed: {}", e),
    }
}

// ── File Watcher ──

#[derive(Debug, Clone, Serialize)]
pub struct FileEvent {
    pub path: String,
    pub kind: String,
    pub timestamp: String,
}

pub fn start_watcher<F>(watch_dir: PathBuf, on_change: F)
where
    F: Fn(FileEvent) + Send + 'static,
{
    if !watch_dir.exists() { std::fs::create_dir_all(&watch_dir).ok(); }

    std::thread::spawn(move || {
        use std::time::{SystemTime, UNIX_EPOCH};
        let mut known: std::collections::HashMap<PathBuf, u128> = std::collections::HashMap::new();

        loop {
            std::thread::sleep(Duration::from_secs(3));
            if let Ok(entries) = std::fs::read_dir(&watch_dir) {
                let mut current: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

                for entry in entries.flatten() {
                    let path = entry.path();
                    current.insert(path.clone());

                    let modified_ns = match entry.metadata()
                        .and_then(|m| m.modified())
                    {
                        Ok(t) => match t.duration_since(UNIX_EPOCH) {
                            Ok(d) => d.as_nanos(),
                            Err(_) => continue,
                        },
                        Err(_) => continue,
                    };

                    if !known.contains_key(&path) {
                        let rel = path.strip_prefix(&watch_dir).unwrap_or(&path).to_string_lossy().to_string();
                        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs().to_string()).unwrap_or_default();
                        on_change(FileEvent { path: rel, kind: "created".into(), timestamp: ts });
                        known.insert(path, modified_ns);
                    } else if known.get(&path) != Some(&modified_ns) {
                        let rel = path.strip_prefix(&watch_dir).unwrap_or(&path).to_string_lossy().to_string();
                        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs().to_string()).unwrap_or_default();
                        on_change(FileEvent { path: rel, kind: "modified".into(), timestamp: ts });
                        known.insert(path, modified_ns);
                    }
                }

                let deleted: Vec<PathBuf> = known.keys()
                    .filter(|k| !current.contains(*k)).cloned().collect();
                for path in deleted {
                    let rel = path.strip_prefix(&watch_dir).unwrap_or(&path).to_string_lossy().to_string();
                    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs().to_string()).unwrap_or_default();
                    on_change(FileEvent { path: rel, kind: "deleted".into(), timestamp: ts });
                    known.remove(&path);
                }
            }
        }
    });
}

// ── Desk Notes ──

#[derive(Debug, Clone, Serialize)]
pub struct DeskNote {
    pub path: String,
    pub title: String,
    pub content: String,
    pub created: String,
}

pub fn list_desk_notes(desk_dir: &PathBuf) -> Vec<DeskNote> {
    let mut notes = Vec::new();
    if !desk_dir.exists() { return notes; }
    if let Ok(entries) = std::fs::read_dir(desk_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let title = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let ts = std::fs::metadata(&path)
                        .and_then(|m| m.created()).ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs().to_string()).unwrap_or_default();
                    notes.push(DeskNote { path: path.to_string_lossy().to_string(), title, content, created: ts });
                }
            }
        }
    }
    notes
}
