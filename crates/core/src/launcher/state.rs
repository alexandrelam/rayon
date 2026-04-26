use crate::catalog::AppCatalog;
use rayon_types::InteractiveSessionMetadata;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Clone)]
pub(crate) struct ActiveInteractiveSession {
    pub provider_index: usize,
    pub metadata: InteractiveSessionMetadata,
}

pub(crate) fn read_app_catalog(
    app_catalog: &RwLock<AppCatalog>,
) -> RwLockReadGuard<'_, AppCatalog> {
    match app_catalog.read() {
        Ok(app_catalog) => app_catalog,
        Err(poisoned) => {
            eprintln!("app catalog lock poisoned while reading");
            poisoned.into_inner()
        }
    }
}

pub(crate) fn write_app_catalog(
    app_catalog: &RwLock<AppCatalog>,
) -> RwLockWriteGuard<'_, AppCatalog> {
    match app_catalog.write() {
        Ok(app_catalog) => app_catalog,
        Err(poisoned) => {
            eprintln!("app catalog lock poisoned while writing");
            poisoned.into_inner()
        }
    }
}

pub(crate) fn read_interactive_sessions(
    sessions: &RwLock<HashMap<String, ActiveInteractiveSession>>,
) -> RwLockReadGuard<'_, HashMap<String, ActiveInteractiveSession>> {
    match sessions.read() {
        Ok(sessions) => sessions,
        Err(poisoned) => {
            eprintln!("interactive session lock poisoned while reading");
            poisoned.into_inner()
        }
    }
}

pub(crate) fn write_interactive_sessions(
    sessions: &RwLock<HashMap<String, ActiveInteractiveSession>>,
) -> RwLockWriteGuard<'_, HashMap<String, ActiveInteractiveSession>> {
    match sessions.write() {
        Ok(sessions) => sessions,
        Err(poisoned) => {
            eprintln!("interactive session lock poisoned while writing");
            poisoned.into_inner()
        }
    }
}

pub(crate) fn next_session_id(counter: &AtomicU64) -> String {
    let next_id = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("session-{next_id}")
}
