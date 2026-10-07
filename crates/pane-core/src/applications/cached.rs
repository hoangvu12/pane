//! The host's list of applications by identity, kept in front of a system
//! adapter, so that a guest asking for the installed applications gets them
//! at once after the first scan, and every id, current or from before
//! identities, finds what opens it.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::identity::{Catalog, is_identity};
use super::{Application, Applications, Discovery};

/// The installed applications as `inner`'s sources last made them
/// ([`Catalog`]). The first call scans, on the calling thread (there is
/// nothing to return before); later calls return the kept list at once and,
/// when it is older than `max_age`, rescan on a thread of their own, so the
/// next call gets the new list. A failed rescan keeps the old list. Nothing
/// is scanned until the first call, so a disabled Applications extension
/// causes no scanning.
pub struct Cached {
    inner: Arc<dyn Discovery>,
    max_age: Duration,
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    /// The last list and when its scan started.
    list: Option<(Arc<Catalog>, Instant)>,
    /// Whether a rescan is running.
    rescanning: bool,
}

impl Cached {
    pub fn new(inner: Arc<dyn Discovery>, max_age: Duration) -> Cached {
        Cached {
            inner,
            max_age,
            state: Arc::default(),
        }
    }

    fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
        state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The kept list, scanned first if there is none; rescanned in the
    /// background once old (see [`Cached`]).
    pub fn catalog(&self) -> Result<Arc<Catalog>, String> {
        let mut state = Cached::lock(&self.state);
        let Some((catalog, scanned)) = &state.list else {
            drop(state);
            let started = Instant::now();
            let catalog = Arc::new(Catalog::new(self.inner.sources()?));
            Cached::lock(&self.state).list = Some((catalog.clone(), started));
            return Ok(catalog);
        };
        let catalog = catalog.clone();
        if scanned.elapsed() >= self.max_age && !state.rescanning {
            state.rescanning = true;
            let (inner, shared) = (self.inner.clone(), self.state.clone());
            std::thread::spawn(move || {
                let started = Instant::now();
                let rescanned = inner.sources();
                let mut state = Cached::lock(&shared);
                state.rescanning = false;
                if let Ok(sources) = rescanned {
                    state.list = Some((Arc::new(Catalog::new(sources)), started));
                }
            });
        }
        Ok(catalog)
    }

    /// The kept list, if there is one; never scans.
    fn kept(&self) -> Option<Arc<Catalog>> {
        Cached::lock(&self.state)
            .list
            .as_ref()
            .map(|(catalog, _)| catalog.clone())
    }

    /// The path of the primary source of the application `id` names, from
    /// the kept list, scanning first when none is kept.
    fn primary_path(&self, id: &str) -> Option<String> {
        let catalog = match self.kept() {
            Some(catalog) => catalog,
            None => self.catalog().ok()?,
        };
        catalog
            .find(id)
            .map(|application| application.primary().path.clone())
    }
}

impl Applications for Cached {
    fn installed(&self) -> Result<Vec<Application>, String> {
        Ok(self.catalog()?.applications())
    }

    fn open(&self, id: &str) -> Result<(), String> {
        if let Some(path) = self.primary_path(id) {
            return self.inner.open(&path);
        }
        if is_identity(id) {
            return Err("it is no longer installed".into());
        }
        // A path from before identities that no application has now: the
        // adapter explains why (it no longer exists, it is not one).
        self.inner.open(id)
    }

    fn source(&self, id: &str) -> Option<String> {
        self.primary_path(id)
    }

    fn current_id(&self, id: &str) -> Option<String> {
        self.kept()?
            .find(id)
            .map(|application| application.id.clone())
    }
}
