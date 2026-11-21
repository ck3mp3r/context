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
fn test_task_status_can_transition_to_forbidden_pairs() {
    let forbidden = [
        (TaskStatus::Backlog, TaskStatus::Done),
        (TaskStatus::Backlog, TaskStatus::Review),
        (TaskStatus::Todo, TaskStatus::Done),
        (TaskStatus::Todo, TaskStatus::Review),
        (TaskStatus::InProgress, TaskStatus::Done),
        (TaskStatus::Done, TaskStatus::Review),
        (TaskStatus::Cancelled, TaskStatus::Done),
        (TaskStatus::Cancelled, TaskStatus::Review),
    ];

    for (source, target) in forbidden {
        assert!(
            !source.can_transition_to(target.clone()),
            "{source} -> {target} must be forbidden"
        );
    }
}

#[test]
fn test_task_status_can_transition_to_identity() {
    let statuses = [
        TaskStatus::Backlog,
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::Review,
        TaskStatus::Done,
        TaskStatus::Cancelled,
    ];

    for status in statuses {
        assert!(
            status.can_transition_to(status.clone()),
            "{status} -> {status} must be allowed"
        );
    }
}

#[test]
fn test_task_status_can_transition_to_allowed_pairs() {
    let allowed = [
        (TaskStatus::Backlog, TaskStatus::Todo),
        (TaskStatus::Backlog, TaskStatus::InProgress),
        (TaskStatus::Backlog, TaskStatus::Cancelled),
        (TaskStatus::Todo, TaskStatus::Backlog),
        (TaskStatus::Todo, TaskStatus::InProgress),
        (TaskStatus::Todo, TaskStatus::Cancelled),
        (TaskStatus::InProgress, TaskStatus::Backlog),
        (TaskStatus::InProgress, TaskStatus::Todo),
        (TaskStatus::InProgress, TaskStatus::Review),
        (TaskStatus::InProgress, TaskStatus::Cancelled),
        (TaskStatus::Review, TaskStatus::Backlog),
        (TaskStatus::Review, TaskStatus::Todo),
        (TaskStatus::Review, TaskStatus::InProgress),
        (TaskStatus::Review, TaskStatus::Done),
        (TaskStatus::Review, TaskStatus::Cancelled),
        (TaskStatus::Done, TaskStatus::Backlog),
        (TaskStatus::Done, TaskStatus::Todo),
        (TaskStatus::Done, TaskStatus::InProgress),
        (TaskStatus::Done, TaskStatus::Cancelled),
        (TaskStatus::Cancelled, TaskStatus::Backlog),
        (TaskStatus::Cancelled, TaskStatus::Todo),
        (TaskStatus::Cancelled, TaskStatus::InProgress),
    ];

    for (source, target) in allowed {
        assert!(
            source.can_transition_to(target.clone()),
            "{source} -> {target} must be allowed"
        );
    }
}
