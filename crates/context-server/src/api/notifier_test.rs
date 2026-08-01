//! Tests for ChangeNotifier pub/sub system.

use super::notifier::{ChangeNotifier, UpdateMessage};

#[tokio::test]
async fn test_multiple_subscribers_receive_same_message() {
    let notifier = ChangeNotifier::new();
    let mut sub1 = notifier.subscribe();
    let mut sub2 = notifier.subscribe();

    let msg = UpdateMessage::NoteCreated {
        note_id: "test123".to_string(),
    };

    notifier.notify(msg.clone());

    let received1 = sub1.recv().await.unwrap();
    let received2 = sub2.recv().await.unwrap();

    assert_eq!(received1, msg);
    assert_eq!(received2, msg);
}

#[tokio::test]
async fn test_notify_with_no_subscribers_does_not_panic() {
    let notifier = ChangeNotifier::new();

    // Should not panic
    notifier.notify(UpdateMessage::ProjectUpdated {
        project_id: "proj123".to_string(),
    });
}

#[tokio::test]
async fn test_all_message_types_are_cloneable() {
    let messages = vec![
        UpdateMessage::NoteCreated {
            note_id: "n1".to_string(),
        },
        UpdateMessage::NoteUpdated {
            note_id: "n2".to_string(),
        },
        UpdateMessage::NoteDeleted {
            note_id: "n3".to_string(),
        },
        UpdateMessage::ProjectCreated {
            project_id: "p1".to_string(),
        },
        UpdateMessage::ProjectUpdated {
            project_id: "p2".to_string(),
        },
        UpdateMessage::ProjectDeleted {
            project_id: "p3".to_string(),
        },
        UpdateMessage::RepoCreated {
            repo_id: "r1".to_string(),
        },
        UpdateMessage::RepoUpdated {
            repo_id: "r2".to_string(),
        },
        UpdateMessage::RepoDeleted {
            repo_id: "r3".to_string(),
        },
        UpdateMessage::TaskListCreated {
            task_list_id: "tl1".to_string(),
        },
        UpdateMessage::TaskListUpdated {
            task_list_id: "tl2".to_string(),
        },
        UpdateMessage::TaskListDeleted {
            task_list_id: "tl3".to_string(),
        },
        UpdateMessage::TaskCreated {
            task_id: "t1".to_string(),
        },
        UpdateMessage::TaskUpdated {
            task_id: "t2".to_string(),
        },
        UpdateMessage::TaskDeleted {
            task_id: "t3".to_string(),
        },
    ];

    // All should clone without error
    for msg in &messages {
        let _cloned = msg.clone();
    }
}

#[tokio::test]
async fn test_messages_are_serializable() {
    let msg = UpdateMessage::TaskCreated {
        task_id: "task123".to_string(),
    };

    // Should serialize to JSON
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("TaskCreated"));
    assert!(json.contains("task123"));

    // Should deserialize back
    let deserialized: UpdateMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, msg);
}

#[tokio::test]
async fn test_subscriber_receives_multiple_messages_in_order() {
    let notifier = ChangeNotifier::new();
    let mut sub = notifier.subscribe();

    let msg1 = UpdateMessage::NoteCreated {
        note_id: "note1".to_string(),
    };
    let msg2 = UpdateMessage::NoteUpdated {
        note_id: "note2".to_string(),
    };
    let msg3 = UpdateMessage::NoteDeleted {
        note_id: "note3".to_string(),
    };

    notifier.notify(msg1.clone());
    notifier.notify(msg2.clone());
    notifier.notify(msg3.clone());

    assert_eq!(sub.recv().await.unwrap(), msg1);
    assert_eq!(sub.recv().await.unwrap(), msg2);
    assert_eq!(sub.recv().await.unwrap(), msg3);
}

#[tokio::test]
async fn test_late_subscriber_does_not_receive_old_messages() {
    let notifier = ChangeNotifier::new();

    // Send message before subscribing
    notifier.notify(UpdateMessage::ProjectCreated {
        project_id: "old_project".to_string(),
    });

    // Subscribe after message sent
    let mut sub = notifier.subscribe();

    // Send new message
    let new_msg = UpdateMessage::ProjectUpdated {
        project_id: "new_project".to_string(),
    };
    notifier.notify(new_msg.clone());

    // Should only receive new message, not old one
    let received = sub.recv().await.unwrap();
    assert_eq!(received, new_msg);

    // No more messages should be available
    assert!(sub.try_recv().is_err());
}
