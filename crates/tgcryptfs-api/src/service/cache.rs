use crate::service::session::SessionManager;

/// Aggregates cache stats from open sessions.
pub struct CacheService;

/// Aggregated cache statistics.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct AggregatedCacheStats {
    pub total_entries: usize,
    pub total_size_bytes: u64,
    pub open_sessions: usize,
}

impl CacheService {
    /// Gather cache stats from all open volume sessions.
    ///
    /// Per-session cache entry/size tracking is not yet implemented;
    /// only the open session count is reported.
    pub async fn aggregate_stats(sessions: &SessionManager) -> AggregatedCacheStats {
        let open = sessions.list_open().await;
        AggregatedCacheStats {
            total_entries: 0,
            total_size_bytes: 0,
            open_sessions: open.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn empty_stats_with_no_sessions() {
        let dir = TempDir::new().unwrap();
        let mgr = SessionManager::new(dir.path().to_path_buf());
        let stats = CacheService::aggregate_stats(&mgr).await;
        assert_eq!(stats.open_sessions, 0);
        assert_eq!(stats.total_entries, 0);
    }
}
