use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::RwLock;

use crate::types::RuntimeArgs;

#[derive(Debug, Deserialize, Clone)]
pub struct RuntimeArgsUpdate {
    pub extra_cli_args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Default)]
pub struct UpdateResult {
    pub restart_needed: bool,
    pub headers_changed: bool,
}

#[derive(Clone, Default)]
pub struct RuntimeArgsStore {
    global: Arc<RwLock<RuntimeArgs>>,
    sessions: Arc<RwLock<HashMap<String, RuntimeArgs>>>,
}

impl RuntimeArgsStore {
    pub fn new(initial: RuntimeArgs) -> Self {
        Self {
            global: Arc::new(RwLock::new(initial)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_global(&self, update: RuntimeArgsUpdate) -> UpdateResult {
        let mut result = UpdateResult::default();
        let mut guard = self.global.write().await;
        if let Some(extra) = update.extra_cli_args {
            guard.extra_cli_args = extra;
            result.restart_needed = true;
        }
        if let Some(env) = update.env {
            guard.env = env;
            result.restart_needed = true;
        }
        if let Some(headers) = update.headers {
            guard.headers = headers;
            result.headers_changed = true;
        }
        result
    }

    pub async fn update_session(&self, session_id: &str, update: RuntimeArgsUpdate) -> UpdateResult {
        let mut result = UpdateResult::default();
        let mut sessions = self.sessions.write().await;
        let entry = sessions.entry(session_id.to_string()).or_default();
        if let Some(extra) = update.extra_cli_args {
            entry.extra_cli_args = extra;
            result.restart_needed = true;
        }
        if let Some(env) = update.env {
            entry.env = env;
            result.restart_needed = true;
        }
        if let Some(headers) = update.headers {
            entry.headers = headers;
            result.headers_changed = true;
        }
        result
    }

    pub async fn get_effective(&self, session_id: Option<&str>) -> RuntimeArgs {
        let global = self.global.read().await.clone();
        if let Some(id) = session_id {
            let sessions = self.sessions.read().await;
            if let Some(overlay) = sessions.get(id) {
                return RuntimeArgs::merge(&global, overlay);
            }
        }
        global
    }

    pub async fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }
}
