use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct RequestLog {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub model_in: String,
    pub model_out: String,
    pub provider: String,
    pub status: u16,
    pub latency_ms: u64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub request_body: Value,
    pub response_body: Value,
}

#[derive(Clone)]
pub struct LogStore {
    logs: Arc<Mutex<VecDeque<RequestLog>>>,
    capacity: usize,
}

impl LogStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            logs: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    pub fn push(&self, log: RequestLog) {
        let mut logs = self.logs.lock().unwrap();
        if logs.len() >= self.capacity {
            logs.pop_front();
        }
        logs.push_back(log);
    }

    pub fn get_all(&self) -> Vec<RequestLog> {
        self.logs.lock().unwrap().iter().cloned().collect()
    }

    pub fn get_recent(&self, limit: usize) -> Vec<RequestLog> {
        let logs = self.logs.lock().unwrap();
        logs.iter().rev().take(limit).cloned().collect()
    }

    pub fn clear(&self) {
        self.logs.lock().unwrap().clear();
    }

    pub fn new_id() -> String {
        Uuid::new_v4().to_string()
    }
}
