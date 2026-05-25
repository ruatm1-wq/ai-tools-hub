/// Process Manager — spawn CLI tools via standard input/output
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::HashMap;

pub struct PtySession {
    pub tool_id: String,
    stdin: Arc<Mutex<Box<dyn Write + Send>>>,
    output_buffer: Arc<Mutex<String>>,
    _reader_thread: Option<thread::JoinHandle<()>>,
}

pub struct PtyManager {
    sessions: Mutex<HashMap<String, PtySession>>,
}

impl PtyManager {
    pub fn new() -> Self {
        PtyManager { sessions: Mutex::new(HashMap::new()) }
    }

    pub fn start_session(&self, tool_id: &str, command: &str, args: &[String]) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(|e| format!("Lock: {}", e))?;
        if let Some(old) = sessions.remove(tool_id) { drop(old); }

        let mut child = Command::new(command);
        for arg in args { child.arg(arg); }
        child.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)] {
            use std::os::windows::process::CommandExt;
            child.creation_flags(0x08000000);
        }

        let mut child = child.spawn().map_err(|e| format!("Spawn: {}", e))?;
        let stdin = child.stdin.take().ok_or("No stdin")?;
        let stdout = child.stdout.take().ok_or("No stdout")?;

        let output_buffer: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let buf = output_buffer.clone();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if let Ok(line) = line {
                    if let Ok(mut b) = buf.lock() { b.push_str(&line); b.push('\n'); }
                } else { break; }
            }
        });

        sessions.insert(tool_id.to_string(), PtySession {
            tool_id: tool_id.to_string(),
            stdin: Arc::new(Mutex::new(Box::new(stdin))),
            output_buffer,
            _reader_thread: Some(reader),
        });
        Ok(())
    }

    pub fn chat(&self, tool_id: &str, message: &str) -> Result<String, String> {
        let sessions = self.sessions.lock().map_err(|e| format!("Lock: {}", e))?;
        let session = sessions.get(tool_id).ok_or_else(|| format!("No session: {}", tool_id))?;
        {
            let mut w = session.stdin.lock().map_err(|e| format!("Lock: {}", e))?;
            writeln!(w, "{}", message).map_err(|e| format!("Write: {}", e))?;
            w.flush().map_err(|e| format!("Flush: {}", e))?;
        }
        let start = Instant::now();
        let mut last_len = 0; let mut stable = 0;
        loop {
            let buf = session.output_buffer.lock().map_err(|e| format!("Buf: {}", e))?;
            let len = buf.len();
            if len > last_len { stable = 0; last_len = len; }
            else { stable += 1; }
            if stable > 5 && len > 0 {
                let r = buf.clone(); drop(buf);
                session.output_buffer.lock().unwrap().clear();
                return Ok(clean_output(&r, message));
            }
            if start.elapsed() > Duration::from_millis(15000) {
                let r = buf.clone(); drop(buf);
                session.output_buffer.lock().unwrap().clear();
                return Ok(clean_output(&r, message));
            }
            drop(buf); thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn close_session(&self, tool_id: &str) { if let Ok(mut s) = self.sessions.lock() { s.remove(tool_id); } }
    pub fn list_sessions(&self) -> Vec<String> {
        self.sessions.lock().map(|s| s.keys().cloned().collect()).unwrap_or_default()
    }
}

fn clean_output(output: &str, sent_message: &str) -> String {
    output.lines().filter(|l| {
        let t = l.trim();
        !t.is_empty() && t != sent_message && !t.contains("$ ") && !t.contains("> ") && !t.contains("PS ")
    }).collect::<Vec<_>>().join("\n").trim().to_string()
}
