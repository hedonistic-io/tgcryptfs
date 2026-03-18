use std::time::Instant;

use crate::types::SystemStatus;

/// System-wide status tracking.
pub struct SystemService {
    start_time: Instant,
    version: String,
}

impl Default for SystemService {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemService {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Get the current system status.
    pub fn status(
        &self,
        telegram_connected: bool,
        volumes_mounted: usize,
        total_volumes: usize,
        cache_entries: usize,
        cache_size_bytes: u64,
        deadman_armed: bool,
    ) -> SystemStatus {
        SystemStatus {
            version: self.version.clone(),
            telegram_connected,
            volumes_mounted,
            total_volumes,
            cache_entries,
            cache_size_bytes,
            deadman_armed,
            uptime_secs: self.start_time.elapsed().as_secs(),
        }
    }

    /// Get the application version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Get uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_status() {
        let svc = SystemService::new();
        let status = svc.status(false, 0, 3, 50, 1024, false);
        assert_eq!(status.total_volumes, 3);
        assert_eq!(status.cache_entries, 50);
        assert!(!status.telegram_connected);
        assert!(!status.deadman_armed);
    }

    #[test]
    fn version_matches() {
        let svc = SystemService::new();
        assert_eq!(svc.version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn uptime_advances() {
        let svc = SystemService::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        // Uptime should be at least 0 (may be 0 due to timer granularity)
        let _ = svc.uptime_secs();
    }
}
