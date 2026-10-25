//! 批量操作的统一结果结构。

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchFailure {
    pub id: String,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchOutcome {
    pub succeeded: usize,
    pub failed: Vec<BatchFailure>,
}

impl BatchOutcome {
    pub fn new() -> Self {
        Self {
            succeeded: 0,
            failed: Vec::new(),
        }
    }

    pub fn record_success(&mut self) {
        self.succeeded += 1;
    }

    pub fn record_failure(&mut self, id: &str, name: &str, message: impl Into<String>) {
        self.failed.push(BatchFailure {
            id: id.to_string(),
            name: name.to_string(),
            message: message.into(),
        });
    }

    pub fn total(&self) -> usize {
        self.succeeded + self.failed.len()
    }
}

impl Default for BatchOutcome {
    fn default() -> Self {
        Self::new()
    }
}
