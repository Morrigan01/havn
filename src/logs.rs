use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

const MAX_LINES: usize = 500;

#[derive(Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub ts: String,
    pub stream: String, // "stdout" | "stderr"
    pub text: String,
}

pub struct LogStore {
    buffers: Mutex<HashMap<i64, VecDeque<LogLine>>>,
}

impl LogStore {
    pub fn new() -> Self {
        Self { buffers: Mutex::new(HashMap::new()) }
    }

    pub fn push(&self, project_id: i64, stream: &str, text: String) {
        let mut buffers = self.buffers.lock().unwrap();
        let buf = buffers.entry(project_id).or_default();
        buf.push_back(LogLine {
            ts: chrono::Utc::now().to_rfc3339(),
            stream: stream.to_string(),
            text,
        });
        while buf.len() > MAX_LINES {
            buf.pop_front();
        }
    }

    pub fn get(&self, project_id: i64, n: usize) -> Vec<LogLine> {
        let buffers = self.buffers.lock().unwrap();
        match buffers.get(&project_id) {
            None => Vec::new(),
            Some(buf) => {
                let skip = if buf.len() > n { buf.len() - n } else { 0 };
                buf.iter().skip(skip).cloned().collect()
            }
        }
    }

    pub fn clear(&self, project_id: i64) {
        let mut buffers = self.buffers.lock().unwrap();
        buffers.remove(&project_id);
    }
}
