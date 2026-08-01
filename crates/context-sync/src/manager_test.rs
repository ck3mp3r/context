use crate::git::MockGitOps;
use crate::manager::*;
use tempfile::TempDir;

#[test]
fn test_is_initialized_false() {
    let temp_dir = TempDir::new().unwrap();
    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    assert!(!manager.is_initialized());
}

#[test]
fn test_is_initialized_true() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();

    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    assert!(manager.is_initialized());
}

#[test]
fn test_entity_counts_includes_skills_field() {
    // Test that EntityCounts struct has skills field
    let counts = EntityCounts {
        repos: 1,
        projects: 2,
        task_lists: 3,
        tasks: 4,
        notes: 5,
        skills: 6,
        attachments: 7,
    };

    assert_eq!(counts.repos, 1);
    assert_eq!(counts.projects, 2);
    assert_eq!(counts.task_lists, 3);
    assert_eq!(counts.tasks, 4);
    assert_eq!(counts.notes, 5);
    assert_eq!(counts.skills, 6);
    assert_eq!(counts.attachments, 7);
    assert_eq!(counts.total(), 28); // Sum of all counts (1+2+3+4+5+6+7)
}
