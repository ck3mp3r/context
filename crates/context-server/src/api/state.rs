//! Application state for the API server.

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use super::Config;
use super::notifier::ChangeNotifier;
use context_core::Database;
use context_core::update::{NoUpdateFlow, UpdateFlow};
use context_sync::{GitOps, SyncManager};

/// Settings the server process actually started with.
///
/// These values come from the [`Config`] handed to `api::run`, so a client can
/// compare them with the stored values and see what a restart would change.
/// `skills_dir` is not here: the state already carries it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunningSettings {
    /// Host address the server bound to.
    pub host: IpAddr,
    /// Port the server bound to.
    pub port: u16,
    /// Logging verbosity of this process (0=warn, 1=info, 2=debug, 3=trace).
    pub verbosity: u8,
    /// Whether this process serves the OpenAPI documentation endpoint.
    pub enable_docs: bool,
    /// Whether this process binds a fixed host and ignores the stored host value.
    pub host_fixed: bool,
}

impl RunningSettings {
    /// Running settings described by `config`.
    pub fn from_config(config: &Config) -> Self {
        Self {
            host: config.host,
            port: config.port,
            verbosity: config.verbosity,
            enable_docs: config.enable_docs,
            host_fixed: config.host_fixed,
        }
    }
}

impl Default for RunningSettings {
    fn default() -> Self {
        Self::from_config(&Config::default())
    }
}

/// Shared application state.
///
/// Contains the database connection, sync manager, change notifier, the update
/// flow, and the settings this process started with.
///
/// # SOLID Principles
///
/// Follows Dependency Inversion Principle (DIP):
/// - Generic over `D: Database` - Can use any database implementation
/// - Generic over `G: GitOps` - Can use RealGit (production) or MockGitOps (tests)
/// - Generic over `U: UpdateFlow` - Can use the real flow or a recording fake
///
/// Dependencies are injected via constructor, not created internally.
pub struct AppState<D: Database, G: GitOps + Send + Sync, U: UpdateFlow = NoUpdateFlow> {
    db: Arc<D>,
    sync_manager: SyncManager<G>,
    notifier: ChangeNotifier,
    skills_dir: PathBuf,
    running: RunningSettings,
    update: Arc<U>,
}

impl<D: Database, G: GitOps + Send + Sync, U: UpdateFlow> Clone for AppState<D, G, U> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            sync_manager: self.sync_manager.clone(),
            notifier: self.notifier.clone(),
            skills_dir: self.skills_dir.clone(),
            running: self.running,
            update: Arc::clone(&self.update),
        }
    }
}

impl<D: Database, G: GitOps + Send + Sync> AppState<D, G, NoUpdateFlow> {
    pub fn new(
        db: D,
        sync_manager: SyncManager<G>,
        notifier: ChangeNotifier,
        skills_dir: PathBuf,
    ) -> Self {
        Self {
            db: Arc::new(db),
            sync_manager,
            notifier,
            skills_dir,
            running: RunningSettings::default(),
            update: Arc::new(NoUpdateFlow),
        }
    }

    /// Attach the flow the update endpoints trigger.
    ///
    /// A state built without it answers update triggers with `Unavailable`.
    pub fn with_update<U: UpdateFlow>(self, update: U) -> AppState<D, G, U> {
        AppState {
            db: self.db,
            sync_manager: self.sync_manager,
            notifier: self.notifier,
            skills_dir: self.skills_dir,
            running: self.running,
            update: Arc::new(update),
        }
    }
}

impl<D: Database, G: GitOps + Send + Sync, U: UpdateFlow> AppState<D, G, U> {
    /// Carry the settings this process started with.
    ///
    /// `api::run` calls this with the [`Config`] it was handed; a state built
    /// without it reports the default config.
    pub fn with_running(mut self, running: RunningSettings) -> Self {
        self.running = running;
        self
    }

    pub fn db(&self) -> &D {
        &self.db
    }

    pub fn db_arc(&self) -> Arc<D> {
        Arc::clone(&self.db)
    }

    pub fn sync_manager(&self) -> &SyncManager<G> {
        &self.sync_manager
    }

    pub fn notifier(&self) -> &ChangeNotifier {
        &self.notifier
    }

    pub fn skills_dir(&self) -> &PathBuf {
        &self.skills_dir
    }

    pub fn running(&self) -> &RunningSettings {
        &self.running
    }

    pub fn update(&self) -> &U {
        &self.update
    }
}
