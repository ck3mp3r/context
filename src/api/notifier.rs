//! Change notification system for broadcasting database updates to WebSocket clients.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Messages broadcast when database entities are created, updated, or deleted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum UpdateMessage {
    // Notes
    NoteCreated { note_id: String },
    NoteUpdated { note_id: String },
    NoteDeleted { note_id: String },

    // Projects
    ProjectCreated { project_id: String },
    ProjectUpdated { project_id: String },
    ProjectDeleted { project_id: String },

    // Repos
    RepoCreated { repo_id: String },
    RepoUpdated { repo_id: String },
    RepoDeleted { repo_id: String },

    // TaskLists
    TaskListCreated { task_list_id: String },
    TaskListUpdated { task_list_id: String },
    TaskListDeleted { task_list_id: String },

    // Tasks
    TaskCreated { task_id: String },
    TaskUpdated { task_id: String },
    TaskDeleted { task_id: String },
}

/// Pub/sub notifier for broadcasting database changes to all subscribers.
#[derive(Clone)]
pub struct ChangeNotifier {
    tx: broadcast::Sender<UpdateMessage>,
}

impl Default for ChangeNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangeNotifier {
    /// Create a new ChangeNotifier with a buffer of 100 messages.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(100);
        Self { tx }
    }

    /// Subscribe to receive update notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<UpdateMessage> {
        self.tx.subscribe()
    }

    /// Broadcast an update message to all subscribers.
    pub fn notify(&self, msg: UpdateMessage) {
        let _ = self.tx.send(msg);
    }
}
