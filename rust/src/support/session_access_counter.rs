use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::support::logger::Logger;

enum SessionState {
    Active { count: usize },
    Timeout { handle: JoinHandle<()> },
}

pub struct SessionAccessCounter {
    timeout: Duration,
    cleanup: Arc<dyn Fn(String) + Send + Sync>,
    logger: Logger,
    sessions: Mutex<HashMap<String, SessionState>>,
}

impl SessionAccessCounter {
    pub fn new(timeout_ms: u64, cleanup: Arc<dyn Fn(String) + Send + Sync>, logger: Logger) -> Self {
        Self {
            timeout: Duration::from_millis(timeout_ms),
            cleanup,
            logger,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub async fn inc(&self, session_id: &str, reason: &str) {
        self.logger
            .info(format!("SessionAccessCounter.inc() {session_id}, caused by {reason}"));
        let mut sessions = self.sessions.lock().await;
        match sessions.remove(session_id) {
            None => {
                self.logger.info(format!(
                    "Session access count 0 -> 1 for {session_id} (new session)"
                ));
                sessions.insert(session_id.to_string(), SessionState::Active { count: 1 });
            }
            Some(SessionState::Timeout { handle }) => {
                handle.abort();
                self.logger.info(format!(
                    "Session access count 0 -> 1, clearing cleanup timeout for {session_id}"
                ));
                sessions.insert(session_id.to_string(), SessionState::Active { count: 1 });
            }
            Some(SessionState::Active { count }) => {
                self.logger.info(format!(
                    "Session access count {count} -> {} for {session_id}",
                    count + 1
                ));
                sessions.insert(
                    session_id.to_string(),
                    SessionState::Active { count: count + 1 },
                );
            }
        }
    }

    pub async fn dec(&self, session_id: &str, reason: &str) {
        self.logger
            .info(format!("SessionAccessCounter.dec() {session_id}, caused by {reason}"));
        let mut sessions = self.sessions.lock().await;
        let Some(state) = sessions.remove(session_id) else {
            self.logger.error(format!(
                "Called dec() on non-existent session {session_id}, ignoring"
            ));
            return;
        };
        match state {
            SessionState::Timeout { .. } => {
                self.logger.error(format!(
                    "Called dec() on session {session_id} that is already pending cleanup, ignoring"
                ));
                sessions.insert(session_id.to_string(), state);
            }
            SessionState::Active { count } => {
                if count == 0 {
                    self.logger.error(format!(
                        "Invalid access count 0 for session {session_id}"
                    ));
                    return;
                }
                let next = count - 1;
                self.logger.info(format!(
                    "Session access count {count} -> {next} for {session_id}"
                ));
                if next == 0 {
                    self.logger.info(format!(
                        "Session access count reached 0, setting cleanup timeout for {session_id}"
                    ));
                    let timeout = self.timeout;
                    let session = session_id.to_string();
                    let cleanup = self.cleanup.clone();
                    let logger = self.logger.clone();
                    let handle = tokio::spawn(async move {
                        tokio::time::sleep(timeout).await;
                        logger.info(format!("Session {session} timed out, cleaning up"));
                        cleanup(session);
                    });
                    sessions.insert(session_id.to_string(), SessionState::Timeout { handle });
                } else {
                    sessions.insert(session_id.to_string(), SessionState::Active { count: next });
                }
            }
        }
    }

    pub async fn clear(&self, session_id: &str, run_cleanup: bool, reason: &str) {
        self.logger
            .info(format!("SessionAccessCounter.clear() {session_id}, caused by {reason}"));
        let mut sessions = self.sessions.lock().await;
        let Some(state) = sessions.remove(session_id) else {
            self.logger.info(format!("Attempted to clear non-existent session {session_id}"));
            return;
        };
        if let SessionState::Timeout { handle } = state {
            handle.abort();
        }
        drop(sessions);
        if run_cleanup {
            (self.cleanup)(session_id.to_string());
        }
    }
}
