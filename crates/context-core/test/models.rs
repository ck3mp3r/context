//! Tests for models.rs.

use crate::models::*;

#[test]
fn test_skill_serde_with_all_fields() {
    let skill = Skill {
        id: "abc12345".to_string(),
        name: "deploy-kubernetes".to_string(),
        description: "Deploy applications to Kubernetes".to_string(),
        content: r#"---
name: deploy-kubernetes
description: Deploy applications to Kubernetes
license: Apache-2.0
compatibility: Requires kubectl, docker
allowed-tools: Bash(kubectl:*) Bash(docker:*)
metadata:
  author: test
  version: "1.0"
origin:
  url: https://github.com/user/repo
  ref: main
  fetched_at: 2026-01-31T10:00:00Z
  imported_by: test
---

# Instructions

Run the deployment scripts...
"#
        .to_string(),
        tags: vec!["kubernetes".to_string(), "deployment".to_string()],
        project_ids: vec!["proj1234".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2026-01-31T10:00:00Z".to_string()),
        updated_at: Some("2026-01-31T10:00:00Z".to_string()),
        deleted_at: None,
        source: Some("git+https://github.com/user/repo".to_string()),
        source_subpath: Some("skills/deploy".to_string()),
    };

    // Test serialization
    let json = serde_json::to_string(&skill).unwrap();
    assert!(json.contains("deploy-kubernetes"));
    assert!(json.contains("content"));

    // Test deserialization
    let deserialized: Skill = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, skill);
}

#[test]
fn test_skill_serde_realistic_minimal() {
    // A realistic minimal skill has full SKILL.md in content field
    let skill = Skill {
        id: "abc12345".to_string(),
        name: "deploy-kubernetes".to_string(),
        description: "Deploy applications to Kubernetes cluster".to_string(),
        content: r#"---
name: deploy-kubernetes
description: Deploy applications to Kubernetes cluster
---

# Deployment Skill

## Steps
1. Validate manifests
2. Apply changes
"#
        .to_string(),
        tags: vec!["kubernetes".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2026-01-31T10:00:00Z".to_string()),
        updated_at: Some("2026-01-31T10:00:00Z".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };

    // Test serialization
    let json = serde_json::to_string(&skill).unwrap();
    assert!(json.contains("deploy-kubernetes"));
    assert!(json.contains("Deploy applications"));

    // Test deserialization
    let deserialized: Skill = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, skill);
}

#[test]
fn test_task_status_workflow_position() {
    assert_eq!(TaskStatus::Backlog.workflow_position(), Some(0));
    assert_eq!(TaskStatus::Todo.workflow_position(), Some(1));
    assert_eq!(TaskStatus::InProgress.workflow_position(), Some(2));
    assert_eq!(TaskStatus::Review.workflow_position(), Some(3));
    assert_eq!(TaskStatus::Done.workflow_position(), Some(4));
    assert_eq!(TaskStatus::Cancelled.workflow_position(), None);
}

#[test]
fn test_task_status_is_terminal_and_is_active() {
    assert!(!TaskStatus::Backlog.is_terminal());
    assert!(!TaskStatus::Todo.is_terminal());
    assert!(!TaskStatus::InProgress.is_terminal());
    assert!(!TaskStatus::Review.is_terminal());
    assert!(TaskStatus::Done.is_terminal());
    assert!(TaskStatus::Cancelled.is_terminal());

    assert!(TaskStatus::Backlog.is_active());
    assert!(TaskStatus::Review.is_active());
    assert!(!TaskStatus::Done.is_active());
    assert!(!TaskStatus::Cancelled.is_active());
}

#[test]
fn test_task_status_forward_step() {
    assert_eq!(TaskStatus::Backlog.forward_step(), Some(TaskStatus::Todo));
    assert_eq!(
        TaskStatus::Todo.forward_step(),
        Some(TaskStatus::InProgress)
    );
    assert_eq!(
        TaskStatus::InProgress.forward_step(),
        Some(TaskStatus::Review)
    );
    assert_eq!(TaskStatus::Review.forward_step(), Some(TaskStatus::Done));
    assert_eq!(TaskStatus::Done.forward_step(), None);
    assert_eq!(TaskStatus::Cancelled.forward_step(), None);
}

#[test]
fn test_task_status_backward_step() {
    assert_eq!(TaskStatus::Backlog.backward_step(), None);
    assert_eq!(TaskStatus::Todo.backward_step(), Some(TaskStatus::Backlog));
    assert_eq!(
        TaskStatus::InProgress.backward_step(),
        Some(TaskStatus::Todo)
    );
    assert_eq!(
        TaskStatus::Review.backward_step(),
        Some(TaskStatus::InProgress)
    );
    assert_eq!(TaskStatus::Done.backward_step(), Some(TaskStatus::Review));
    assert_eq!(TaskStatus::Cancelled.backward_step(), None);
}

#[test]
fn test_task_status_expedite_forward() {
    assert_eq!(
        TaskStatus::Backlog.expedite_forward(),
        Some(TaskStatus::InProgress)
    );
    assert_eq!(
        TaskStatus::InProgress.expedite_forward(),
        Some(TaskStatus::Done)
    );
    assert_eq!(TaskStatus::Todo.expedite_forward(), None);
    assert_eq!(TaskStatus::Review.expedite_forward(), None);
    assert_eq!(TaskStatus::Done.expedite_forward(), None);
    assert_eq!(TaskStatus::Cancelled.expedite_forward(), None);
}

#[test]
fn test_task_status_active_states() {
    assert_eq!(
        TaskStatus::active_states(),
        vec![
            TaskStatus::Backlog,
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::Review,
        ]
    );
}
