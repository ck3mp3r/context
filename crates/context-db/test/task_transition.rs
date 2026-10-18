use crate::SqliteDatabase;
use crate::helpers::read_deleted_at;
use context_core::{
    Database, HasTaskLists, HasTasks, HasTransitionLogs, Task, TaskList, TaskListRepository,
    TaskListStatus, TaskRepository, TaskStatus,
};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    // Create a test project with known ID for tests
    sqlx::query("INSERT OR IGNORE INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("test0000")
        .bind("Test Project")
        .bind("Default project for tests")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Create test project should succeed");

    db
}

fn make_task_list(id: &str, title: &str) -> TaskList {
    TaskList {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: "test0000".to_string(), // Test project (created by setup_db)
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    }
}

fn make_task(id: &str, list_id: &str, title: &str) -> Task {
    Task {
        id: id.to_string(),
        list_id: list_id.to_string(),
        parent_id: None,
        title: title.to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    }
}

// =============================================================================
// Bulk Task Transition Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_success() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup: Create task list
    task_lists
        .create(&make_task_list("bulklist", "Bulk Test"))
        .await
        .expect("Create task list should succeed");

    // Create 3 tasks all with status backlog
    let mut task1 = make_task("bulktsk1", "bulklist", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    let mut task2 = make_task("bulktsk2", "bulklist", "Task 2");
    task2.status = TaskStatus::Backlog;
    tasks
        .create(&task2)
        .await
        .expect("Create task2 should succeed");

    let mut task3 = make_task("bulktsk3", "bulklist", "Task 3");
    task3.status = TaskStatus::Backlog;
    tasks
        .create(&task3)
        .await
        .expect("Create task3 should succeed");

    // Transition all 3 tasks from backlog to in_progress
    let task_ids = vec![
        "bulktsk1".to_string(),
        "bulktsk2".to_string(),
        "bulktsk3".to_string(),
    ];
    let updated_tasks = tasks
        .transition_tasks(&task_ids, TaskStatus::InProgress)
        .await
        .expect("Bulk transition should succeed");

    // Verify all tasks were updated
    assert_eq!(updated_tasks.len(), 3);
    for task in updated_tasks {
        assert_eq!(task.status, TaskStatus::InProgress);
    }

    // Verify via get
    let task1_after = tasks
        .get("bulktsk1")
        .await
        .expect("Get task1 should succeed");
    let task2_after = tasks
        .get("bulktsk2")
        .await
        .expect("Get task2 should succeed");
    let task3_after = tasks
        .get("bulktsk3")
        .await
        .expect("Get task3 should succeed");

    assert_eq!(task1_after.status, TaskStatus::InProgress);
    assert_eq!(task2_after.status, TaskStatus::InProgress);
    assert_eq!(task3_after.status, TaskStatus::InProgress);
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_mixed_status_fails() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("mixlist1", "Mixed Status Test"))
        .await
        .expect("Create task list should succeed");

    // Create tasks with different statuses
    let mut task1 = make_task("mixtsk01", "mixlist1", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    let mut task2 = make_task("mixtsk02", "mixlist1", "Task 2");
    task2.status = TaskStatus::Todo;
    tasks
        .create(&task2)
        .await
        .expect("Create task2 should succeed");

    // Try to transition tasks with different statuses
    let task_ids = vec!["mixtsk01".to_string(), "mixtsk02".to_string()];
    let result = tasks
        .transition_tasks(&task_ids, TaskStatus::InProgress)
        .await;

    assert!(
        result.is_err(),
        "Should fail when tasks have different statuses"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("same current status") || err_msg.contains("mixed status"),
        "Error should mention status mismatch"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_invalid_transition_fails() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("invlist1", "Invalid Transition Test"))
        .await
        .expect("Create task list should succeed");

    // Create tasks with backlog status
    let mut task1 = make_task("invtsk01", "invlist1", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    let mut task2 = make_task("invtsk02", "invlist1", "Task 2");
    task2.status = TaskStatus::Backlog;
    tasks
        .create(&task2)
        .await
        .expect("Create task2 should succeed");

    // Try invalid transition: backlog -> review (not allowed)
    let task_ids = vec!["invtsk01".to_string(), "invtsk02".to_string()];
    let result = tasks.transition_tasks(&task_ids, TaskStatus::Review).await;

    assert!(result.is_err(), "Should fail for invalid transition");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("invalid_transition") || err_msg.contains("not allowed"),
        "Error should mention invalid transition"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_not_found_fails() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("notflist", "Not Found Test"))
        .await
        .expect("Create task list should succeed");

    // Create only one task
    let mut task1 = make_task("notftsk1", "notflist", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    // Try to transition with one non-existent task ID
    let task_ids = vec!["notftsk1".to_string(), "nonexist".to_string()];
    let result = tasks.transition_tasks(&task_ids, TaskStatus::Todo).await;

    assert!(result.is_err(), "Should fail when task not found");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not found") || err_msg.contains("NotFound"),
        "Error should mention task not found"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_transaction_rollback() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("rolllist", "Rollback Test"))
        .await
        .expect("Create task list should succeed");

    // Create tasks
    let mut task1 = make_task("rolltsk1", "rolllist", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    let mut task2 = make_task("rolltsk2", "rolllist", "Task 2");
    task2.status = TaskStatus::Backlog;
    tasks
        .create(&task2)
        .await
        .expect("Create task2 should succeed");

    // Try bulk transition with one non-existent task - should rollback
    let task_ids = vec![
        "rolltsk1".to_string(),
        "rolltsk2".to_string(),
        "nonexist".to_string(),
    ];
    let result = tasks.transition_tasks(&task_ids, TaskStatus::Todo).await;

    assert!(result.is_err(), "Should fail due to non-existent task");

    // Verify no tasks were updated (transaction rolled back)
    let task1_after = tasks
        .get("rolltsk1")
        .await
        .expect("Get task1 should succeed");
    let task2_after = tasks
        .get("rolltsk2")
        .await
        .expect("Get task2 should succeed");

    assert_eq!(
        task1_after.status,
        TaskStatus::Backlog,
        "Task1 should remain backlog"
    );
    assert_eq!(
        task2_after.status,
        TaskStatus::Backlog,
        "Task2 should remain backlog"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_creation_logs_initial_backlog_transition() {
    let db = setup_db().await;

    // Create task list
    let list = make_task_list("lst00004", "Test List");
    db.task_lists().create(&list).await.unwrap();

    // Create task (always starts as backlog)
    let task = make_task("tsk00004", "lst00004", "Test Task");
    let created = db.tasks().create(&task).await.unwrap();

    // Verify transition log was created
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();

    assert_eq!(
        transitions.len(),
        1,
        "Should have exactly one transition log entry"
    );
    assert_eq!(transitions[0].task_id, created.id);
    assert_eq!(
        transitions[0].status,
        TaskStatus::Backlog,
        "Initial transition should be backlog"
    );
    assert!(
        !transitions[0].transitioned_at.is_empty(),
        "Should have timestamp"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_transition_logs_state_change() {
    let db = setup_db().await;

    // Create task list
    let list = make_task_list("lst00005", "Test List");
    db.task_lists().create(&list).await.unwrap();

    // Create task (starts as backlog)
    let task = make_task("tsk00005", "lst00005", "Test Task");
    let created = db.tasks().create(&task).await.unwrap();

    // Should have 1 transition (initial backlog)
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(transitions.len(), 1);

    // Transition to todo
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Todo)
        .await
        .unwrap();

    // Should have 2 transitions now
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(
        transitions.len(),
        2,
        "Should have 2 transitions after todo transition"
    );

    // Transition to in_progress
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::InProgress)
        .await
        .unwrap();

    // Should have 3 transitions now
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(transitions.len(), 3, "Should have 3 transitions");

    // Transition to done
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Done)
        .await
        .unwrap();

    // Should have 4 transitions - complete history
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(
        transitions.len(),
        4,
        "Should have complete transition history"
    );

    // Verify all expected statuses are present (order may vary due to same timestamps)
    let statuses: Vec<TaskStatus> = transitions.iter().map(|t| t.status.clone()).collect();
    assert!(
        statuses.contains(&TaskStatus::Backlog),
        "Should have Backlog transition"
    );
    assert!(
        statuses.contains(&TaskStatus::Todo),
        "Should have Todo transition"
    );
    assert!(
        statuses.contains(&TaskStatus::InProgress),
        "Should have InProgress transition"
    );
    assert!(
        statuses.contains(&TaskStatus::Done),
        "Should have Done transition"
    );
}

// =============================================================================
// Integration Tests - Complete Workflows
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_complete_workflow_through_all_valid_states() {
    // Integration test: Create task and transition through all valid statuses
    let db = setup_db().await;

    // Setup
    let list = make_task_list("lstwork1", "Workflow Test List");
    db.task_lists().create(&list).await.unwrap();

    // Create task (backlog)
    let task = make_task("tskwork1", "lstwork1", "Complete Workflow Task");
    let created = db.tasks().create(&task).await.unwrap();
    assert_eq!(created.status, TaskStatus::Backlog);

    // Transition: backlog → todo
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Todo)
        .await
        .unwrap();
    let task = db.tasks().get(&created.id).await.unwrap();
    assert_eq!(task.status, TaskStatus::Todo);

    // Transition: todo → in_progress
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::InProgress)
        .await
        .unwrap();
    let task = db.tasks().get(&created.id).await.unwrap();
    assert_eq!(task.status, TaskStatus::InProgress);

    // Transition: in_progress → review
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Review)
        .await
        .unwrap();
    let task = db.tasks().get(&created.id).await.unwrap();
    assert_eq!(task.status, TaskStatus::Review);

    // Transition: review → done
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Done)
        .await
        .unwrap();
    let task = db.tasks().get(&created.id).await.unwrap();
    assert_eq!(task.status, TaskStatus::Done);

    // Verify complete history - should have 5 transitions
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(
        transitions.len(),
        5,
        "Should have 5 transitions: backlog → todo → in_progress → review → done"
    );

    // Verify all statuses are present
    let statuses: Vec<TaskStatus> = transitions.iter().map(|t| t.status.clone()).collect();
    assert!(statuses.contains(&TaskStatus::Backlog));
    assert!(statuses.contains(&TaskStatus::Todo));
    assert!(statuses.contains(&TaskStatus::InProgress));
    assert!(statuses.contains(&TaskStatus::Review));
    assert!(statuses.contains(&TaskStatus::Done));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_bulk_transition_logs_all_task_transitions() {
    // Integration test: Bulk transition multiple tasks, verify all get transition logs
    let db = setup_db().await;

    // Setup
    let list = make_task_list("lstbulk1", "Bulk Test List");
    db.task_lists().create(&list).await.unwrap();

    // Create 5 tasks
    let mut task_ids = Vec::new();
    for i in 1..=5 {
        let task = make_task(
            &format!("tskbulk{}", i),
            "lstbulk1",
            &format!("Bulk Task {}", i),
        );
        let created = db.tasks().create(&task).await.unwrap();
        task_ids.push(created.id);
    }

    // All tasks should have 1 transition (initial backlog)
    for task_id in &task_ids {
        let transitions = db.transition_logs().list_by_task_id(task_id).await.unwrap();
        assert_eq!(transitions.len(), 1, "Initial transition logged");
    }

    // Bulk transition: backlog → todo
    db.tasks()
        .transition_tasks(&task_ids, TaskStatus::Todo)
        .await
        .unwrap();

    // Verify all tasks transitioned
    for task_id in &task_ids {
        let task = db.tasks().get(task_id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Todo);

        // Verify transition logged
        let transitions = db.transition_logs().list_by_task_id(task_id).await.unwrap();
        assert_eq!(transitions.len(), 2, "Should have 2 transitions");
        let statuses: Vec<TaskStatus> = transitions.iter().map(|t| t.status.clone()).collect();
        assert!(statuses.contains(&TaskStatus::Backlog));
        assert!(statuses.contains(&TaskStatus::Todo));
    }

    // Bulk transition: todo → in_progress
    db.tasks()
        .transition_tasks(&task_ids, TaskStatus::InProgress)
        .await
        .unwrap();

    // Verify all tasks have 3 transitions
    for task_id in &task_ids {
        let task = db.tasks().get(task_id).await.unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);

        let transitions = db.transition_logs().list_by_task_id(task_id).await.unwrap();
        assert_eq!(
            transitions.len(),
            3,
            "Should have 3 transitions after bulk in_progress"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_task_delete_cascades_to_transitions() {
    // Integration test: Deleting a task should cascade delete its transitions
    let db = setup_db().await;

    // Setup
    let list = make_task_list("lstdel01", "Delete Test List");
    db.task_lists().create(&list).await.unwrap();

    // Create task and transition it
    let task = make_task("tskdel01", "lstdel01", "Task To Delete");
    let created = db.tasks().create(&task).await.unwrap();

    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Todo)
        .await
        .unwrap();
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::InProgress)
        .await
        .unwrap();

    // Verify transitions exist
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(
        transitions.len(),
        3,
        "Should have 3 transitions before delete"
    );

    // Delete task
    db.tasks().delete(&created.id).await.unwrap();

    // Verify task is soft-deleted (tombstone), not hard-deleted
    assert!(
        read_deleted_at(db.pool(), "task", &created.id)
            .await
            .is_some(),
        "Task should be soft-deleted"
    );

    // Verify transitions are cascade deleted
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(
        transitions.len(),
        0,
        "Transitions should be cascade deleted with task"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rapid_transitions_all_logged() {
    // Edge case: Rapid transitions in quick succession should all be logged
    let db = setup_db().await;

    // Setup
    let list = make_task_list("lstrapi1", "Rapid Test List");
    db.task_lists().create(&list).await.unwrap();

    let task = make_task("tskrapi1", "lstrapi1", "Rapid Transition Task");
    let created = db.tasks().create(&task).await.unwrap();

    // Perform rapid transitions
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Todo)
        .await
        .unwrap();
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::InProgress)
        .await
        .unwrap();
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Review)
        .await
        .unwrap();
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Done)
        .await
        .unwrap();

    // Verify all transitions logged despite rapid execution
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(
        transitions.len(),
        5,
        "All rapid transitions should be logged"
    );

    // Verify completeness - all expected statuses present
    let statuses: Vec<TaskStatus> = transitions.iter().map(|t| t.status.clone()).collect();
    assert!(statuses.contains(&TaskStatus::Backlog));
    assert!(statuses.contains(&TaskStatus::Todo));
    assert!(statuses.contains(&TaskStatus::InProgress));
    assert!(statuses.contains(&TaskStatus::Review));
    assert!(statuses.contains(&TaskStatus::Done));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_rollback_on_failure() {
    // Edge case: If transition fails, no transition log should be created
    let db = setup_db().await;

    // Setup
    let list = make_task_list("lstroll1", "Rollback Test List");
    db.task_lists().create(&list).await.unwrap();

    let task = make_task("tskroll1", "lstroll1", "Rollback Test Task");
    let created = db.tasks().create(&task).await.unwrap();

    // Verify initial state
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(transitions.len(), 1, "Should have initial transition");

    // Attempt invalid transition: backlog → done (should fail)
    let result = db
        .tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Done)
        .await;
    assert!(result.is_err(), "Invalid transition should fail validation");

    // Verify no new transition was logged (transaction rolled back)
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(
        transitions.len(),
        1,
        "Failed transition should not create log entry"
    );

    // Verify task status unchanged
    let task = db.tasks().get(&created.id).await.unwrap();
    assert_eq!(
        task.status,
        TaskStatus::Backlog,
        "Task status should be unchanged after failed transition"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cancelled_workflow_preserves_history() {
    // Integration test: Task cancelled mid-workflow preserves full history
    let db = setup_db().await;

    // Setup
    let list = make_task_list("lstcanc1", "Cancelled Test List");
    db.task_lists().create(&list).await.unwrap();

    let task = make_task("tskcanc1", "lstcanc1", "Task To Cancel");
    let created = db.tasks().create(&task).await.unwrap();

    // Progress through some states
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Todo)
        .await
        .unwrap();
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::InProgress)
        .await
        .unwrap();

    // Cancel mid-workflow
    db.tasks()
        .transition_tasks(std::slice::from_ref(&created.id), TaskStatus::Cancelled)
        .await
        .unwrap();

    // Verify complete history preserved
    let transitions = db
        .transition_logs()
        .list_by_task_id(&created.id)
        .await
        .unwrap();
    assert_eq!(
        transitions.len(),
        4,
        "Should have complete history including cancellation"
    );

    let statuses: Vec<TaskStatus> = transitions.iter().map(|t| t.status.clone()).collect();
    assert!(statuses.contains(&TaskStatus::Backlog));
    assert!(statuses.contains(&TaskStatus::Todo));
    assert!(statuses.contains(&TaskStatus::InProgress));
    assert!(statuses.contains(&TaskStatus::Cancelled));

    // Verify final state
    let task = db.tasks().get(&created.id).await.unwrap();
    assert_eq!(task.status, TaskStatus::Cancelled);
}

// =============================================================================
// Guardrail Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn transition_same_status_is_noop() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("grdlist1", "Guardrail List"))
        .await
        .unwrap();

    let task = make_task("grdtsk01", "grdlist1", "Task");
    db.tasks().create(&task).await.unwrap();

    // Transitioning to the same status (backlog → backlog) should succeed silently
    let result = db
        .tasks()
        .transition_tasks(std::slice::from_ref(&task.id), TaskStatus::Backlog)
        .await;
    assert!(result.is_ok(), "Same-status transition should be a no-op");

    // Status should still be backlog
    let fetched = db.tasks().get(&task.id).await.unwrap();
    assert_eq!(fetched.status, TaskStatus::Backlog);
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_done_blocked_by_in_flight_subtasks() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("grdlist2", "Guardrail List 2"))
        .await
        .unwrap();

    // Create parent task in in_progress
    let mut parent = make_task("grdpar01", "grdlist2", "Parent");
    parent.status = TaskStatus::InProgress;
    db.tasks().create(&parent).await.unwrap();

    // Create subtask still in_progress
    let mut subtask = make_task("grdsub01", "grdlist2", "Subtask");
    subtask.parent_id = Some("grdpar01".to_string());
    subtask.status = TaskStatus::InProgress;
    db.tasks().create(&subtask).await.unwrap();

    // Trying to mark parent done should be blocked
    let result = db
        .tasks()
        .transition_tasks(std::slice::from_ref(&parent.id), TaskStatus::Done)
        .await;
    assert!(result.is_err(), "Should be blocked by in-flight subtask");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("in flight") || err.contains("subtask"),
        "Error should mention subtasks: {err}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_cancelled_blocked_by_in_flight_subtasks() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("grdlist3", "Guardrail List 3"))
        .await
        .unwrap();

    let mut parent = make_task("grdpar02", "grdlist3", "Parent");
    parent.status = TaskStatus::InProgress;
    db.tasks().create(&parent).await.unwrap();

    let mut subtask = make_task("grdsub02", "grdlist3", "Subtask");
    subtask.parent_id = Some("grdpar02".to_string());
    subtask.status = TaskStatus::Todo;
    db.tasks().create(&subtask).await.unwrap();

    let result = db
        .tasks()
        .transition_tasks(std::slice::from_ref(&parent.id), TaskStatus::Cancelled)
        .await;
    assert!(result.is_err(), "Should be blocked by in-flight subtask");
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_done_allowed_when_subtasks_completed() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("grdlist4", "Guardrail List 4"))
        .await
        .unwrap();

    let mut parent = make_task("grdpar03", "grdlist4", "Parent");
    parent.status = TaskStatus::InProgress;
    db.tasks().create(&parent).await.unwrap();

    // Subtask is already done
    let mut subtask = make_task("grdsub03", "grdlist4", "Subtask");
    subtask.parent_id = Some("grdpar03".to_string());
    subtask.status = TaskStatus::Done;
    db.tasks().create(&subtask).await.unwrap();

    // Parent should now be transitionable to done
    let result = db
        .tasks()
        .transition_tasks(std::slice::from_ref(&parent.id), TaskStatus::Done)
        .await;
    assert!(
        result.is_ok(),
        "Should allow done when all subtasks complete"
    );
}
