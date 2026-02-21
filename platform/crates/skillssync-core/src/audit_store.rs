use crate::error::SyncEngineError;
use crate::models::{AuditEvent, AuditEventStatus};
use crate::paths::SyncPaths;
use serde::{Deserialize, Serialize};

pub const DEFAULT_AUDIT_LOG_LIMIT: usize = 5000;

#[derive(Debug, Clone)]
pub struct SyncAuditStore {
    paths: SyncPaths,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditLogPayload {
    version: u32,
    #[serde(default)]
    events: Vec<AuditEvent>,
}

impl Default for SyncAuditStore {
    fn default() -> Self {
        Self {
            paths: SyncPaths::detect(),
        }
    }
}

impl SyncAuditStore {
    pub fn new(paths: SyncPaths) -> Self {
        Self { paths }
    }

    pub fn load_events(&self) -> Vec<AuditEvent> {
        let Ok(data) = std::fs::read(&self.paths.audit_log_path) else {
            return Vec::new();
        };

        serde_json::from_slice::<AuditLogPayload>(&data)
            .map(|payload| payload.events)
            .unwrap_or_default()
    }

    pub fn append_event(
        &self,
        event: AuditEvent,
        max_events: usize,
    ) -> Result<(), SyncEngineError> {
        let mut events = self.load_events();
        events.push(event);

        if max_events > 0 && events.len() > max_events {
            let drop_count = events.len() - max_events;
            events.drain(0..drop_count);
        }

        self.save_events(&events)
    }

    pub fn list_events(
        &self,
        limit: Option<usize>,
        status_filter: Option<AuditEventStatus>,
        action_filter: Option<&str>,
    ) -> Vec<AuditEvent> {
        let normalized_action = action_filter
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        let max = limit.unwrap_or(200).max(1);

        self.load_events()
            .into_iter()
            .rev()
            .filter(|event| {
                status_filter
                    .map(|status| event.status == status)
                    .unwrap_or(true)
            })
            .filter(|event| {
                normalized_action
                    .as_ref()
                    .map(|needle| event.action.to_ascii_lowercase().contains(needle))
                    .unwrap_or(true)
            })
            .take(max)
            .collect()
    }

    fn save_events(&self, events: &[AuditEvent]) -> Result<(), SyncEngineError> {
        self.paths
            .ensure_runtime_dir()
            .map_err(|error| SyncEngineError::io(&self.paths.runtime_directory, error))?;
        let payload = AuditLogPayload {
            version: 1,
            events: events.to_vec(),
        };
        let mut data = serde_json::to_vec_pretty(&payload)?;
        data.push(b'\n');
        std::fs::write(&self.paths.audit_log_path, data)
            .map_err(|error| SyncEngineError::io(&self.paths.audit_log_path, error))
    }
}

#[cfg(test)]
mod tests {
    use super::SyncAuditStore;
    use crate::models::{AuditEvent, AuditEventStatus};
    use crate::paths::SyncPaths;
    use tempfile::tempdir;

    fn event(id: usize, action: &str, status: AuditEventStatus) -> AuditEvent {
        AuditEvent {
            id: format!("event-{id}"),
            occurred_at: String::from("2026-02-21T12:00:00Z"),
            action: action.to_string(),
            status,
            trigger: Some(String::from("manual")),
            summary: format!("summary-{id}"),
            paths: vec![format!("/tmp/path-{id}")],
            details: None,
        }
    }

    #[test]
    fn append_event_keeps_ring_limit() {
        let dir = tempdir().expect("tempdir");
        let store = SyncAuditStore::new(SyncPaths::from_runtime(dir.path().to_path_buf()));
        let limit = 10usize;

        for id in 0..(limit + 10) {
            store
                .append_event(event(id, "run_sync", AuditEventStatus::Success), limit)
                .expect("append");
        }

        let events = store.load_events();
        assert_eq!(events.len(), limit);
        assert_eq!(
            events.first().map(|item| item.id.as_str()),
            Some("event-10")
        );
        assert_eq!(events.last().map(|item| item.id.as_str()), Some("event-19"));
    }

    #[test]
    fn list_events_filters_by_status_and_action() {
        let dir = tempdir().expect("tempdir");
        let store = SyncAuditStore::new(SyncPaths::from_runtime(dir.path().to_path_buf()));

        store
            .append_event(event(1, "run_sync", AuditEventStatus::Success), 100)
            .expect("append");
        store
            .append_event(event(2, "run_sync", AuditEventStatus::Failed), 100)
            .expect("append");
        store
            .append_event(
                event(3, "set_mcp_server_enabled", AuditEventStatus::Blocked),
                100,
            )
            .expect("append");

        let failed_sync =
            store.list_events(Some(10), Some(AuditEventStatus::Failed), Some("run_sync"));
        assert_eq!(failed_sync.len(), 1);
        assert_eq!(failed_sync[0].id, "event-2");
    }
}
