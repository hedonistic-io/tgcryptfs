use std::path::PathBuf;
use std::sync::Arc;

use crate::service::auth::AuthService;
use crate::service::deadman::DeadmanService;
use crate::service::session::SessionManager;
use crate::service::system::SystemService;
use crate::service::volume::VolumeService;

/// Shared application state, wrapped in Arc for axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub system: SystemService,
    pub volumes: VolumeService,
    pub auth: AuthService,
    pub sessions: SessionManager,
    pub deadman: DeadmanService,
}

impl AppState {
    pub fn new(volumes_dir: PathBuf, auth: AuthService) -> Self {
        Self {
            inner: Arc::new(AppStateInner {
                system: SystemService::new(),
                volumes: VolumeService::new(volumes_dir.clone()),
                auth,
                sessions: SessionManager::new(volumes_dir),
                deadman: DeadmanService::new(),
            }),
        }
    }
}
