//! Tests for SqliteProjectRepository.

use crate::db::{Database, Project, ProjectQuery, ProjectRepository, SqliteDatabase};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn create_and_get_project() {
    let db = setup_db().await;
    let repo = db.projects();

    let project = Project {
        id: "12345678".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    repo.create(&project).await.expect("Create should succeed");

    let retrieved = repo.get("12345678").await.expect("Get should succeed");
    assert_eq!(retrieved.id, project.id);
    assert_eq!(retrieved.title, project.title);
    assert_eq!(retrieved.description, project.description);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_nonexistent_project_returns_not_found() {
    let db = setup_db().await;
    let repo = db.projects();

    let result = repo.get("nonexist").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn list_projects_includes_created() {
    let db = setup_db().await;
    let repo = db.projects();

    // Initially empty - no default project
    let result = repo.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 0);

    // Create another project
    let project = Project {
        id: "abcd1234".to_string(),
        title: "My Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    repo.create(&project).await.expect("Create should succeed");

    let result = repo.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1); // Just the one we created
    assert!(result.items.iter().any(|p| p.title == "My Project"));
}

#[tokio::test(flavor = "multi_thread")]
async fn update_project() {
    let db = setup_db().await;
    let repo = db.projects();

    let mut project = Project {
        id: "update01".to_string(),
        title: "Original".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    repo.create(&project).await.expect("Create should succeed");

    project.title = "Updated".to_string();
    project.description = Some("Now with description".to_string());
    repo.update(&project).await.expect("Update should succeed");

    let retrieved = repo.get("update01").await.expect("Get should succeed");
    assert_eq!(retrieved.title, "Updated");
    assert_eq!(
        retrieved.description,
        Some("Now with description".to_string())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_project() {
    let db = setup_db().await;
    let repo = db.projects();

    let project = Project {
        id: "delete01".to_string(),
        title: "To Delete".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    repo.create(&project).await.expect("Create should succeed");

    repo.delete("delete01")
        .await
        .expect("Delete should succeed");

    let result = repo.get("delete01").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn project_create_with_tags() {
    let db = setup_db().await;
    let repo = db.projects();

    let project = Project {
        id: "tagproj1".to_string(),
        title: "Tagged Project".to_string(),
        description: None,
        tags: vec!["rust".to_string(), "backend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    repo.create(&project).await.expect("Create should succeed");

    let retrieved = repo.get("tagproj1").await.expect("Get should succeed");
    assert_eq!(retrieved.tags.len(), 2);
    assert!(retrieved.tags.contains(&"rust".to_string()));
    assert!(retrieved.tags.contains(&"backend".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn project_list_with_tag_filter() {
    let db = setup_db().await;
    let repo = db.projects();

    // Create projects with different tags
    repo.create(&Project {
        id: "tagflt01".to_string(),
        title: "Rust Backend".to_string(),
        description: None,
        tags: vec!["rust".to_string(), "backend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "tagflt02".to_string(),
        title: "Rust Frontend".to_string(),
        description: None,
        tags: vec!["rust".to_string(), "frontend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "tagflt03".to_string(),
        title: "Python Backend".to_string(),
        description: None,
        tags: vec!["python".to_string(), "backend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:02".to_string()),
        updated_at: Some("2025-01-01 00:00:02".to_string()),
    })
    .await
    .unwrap();

    // Filter by "rust" tag - should find 2
    let query = ProjectQuery {
        tags: Some(vec!["rust".to_string()]),
        ..Default::default()
    };
    let result = repo.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    // Filter by "backend" tag - should find 2
    let query = ProjectQuery {
        tags: Some(vec!["backend".to_string()]),
        ..Default::default()
    };
    let result = repo.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    // Filter by "python" tag - should find 1
    let query = ProjectQuery {
        tags: Some(vec!["python".to_string()]),
        ..Default::default()
    };
    let result = repo.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Python Backend");
}

#[tokio::test(flavor = "multi_thread")]
async fn project_get_loads_all_relationships() {
    let db = setup_db().await;
    let projects = db.projects();

    // Create project FIRST (required for task_list FK)
    let project = Project {
        id: "projrel1".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    projects
        .create(&project)
        .await
        .expect("Create should succeed");

    // Create repos (for foreign key constraints)
    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("repo0001")
        .bind("https://github.com/test/repo1")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert repo should succeed");

    // Create task list WITH project_id (NOT NULL constraint)
    sqlx::query("INSERT INTO task_list (id, title, description, notes, tags, external_refs, status, project_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind("list0001")
        .bind("Test List")
        .bind(None::<String>)
        .bind(None::<String>)
        .bind("[]")
        .bind("[]")
        .bind("active")
        .bind("projrel1")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert task list should succeed");

    // Create note
    sqlx::query("INSERT INTO note (id, title, content, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("note0001")
        .bind("Test Note")
        .bind("Content")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert note should succeed");

    // Insert relationships into junction tables
    sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
        .bind("projrel1")
        .bind("repo0001")
        .execute(db.pool())
        .await
        .expect("Insert project_repo should succeed");

    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("projrel1")
        .bind("note0001")
        .execute(db.pool())
        .await
        .expect("Insert project_note should succeed");

    // Get project and verify relationships are loaded
    let retrieved = projects.get("projrel1").await.expect("Get should succeed");

    assert_eq!(
        retrieved.repo_ids.len(),
        1,
        "Should load 1 repo relationship"
    );
    assert!(retrieved.repo_ids.contains(&"repo0001".to_string()));

    assert_eq!(
        retrieved.task_list_ids.len(),
        1,
        "Should load 1 task list relationship"
    );
    assert!(retrieved.task_list_ids.contains(&"list0001".to_string()));

    assert_eq!(
        retrieved.note_ids.len(),
        1,
        "Should load 1 note relationship"
    );
    assert!(retrieved.note_ids.contains(&"note0001".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_external_ref() {
    let db = setup_db().await;
    let repo = db.projects();

    let project = Project {
        id: "extref01".to_string(),
        title: "Project with External Ref".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec!["JIRA-123".to_string()],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    repo.create(&project).await.expect("Create should succeed");

    let retrieved = repo.get("extref01").await.expect("Get should succeed");
    assert_eq!(retrieved.external_refs, vec!["JIRA-123".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project_external_ref() {
    let db = setup_db().await;
    let repo = db.projects();

    // Create project without external_ref
    let mut project = Project {
        id: "extref02".to_string(),
        title: "Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    repo.create(&project).await.expect("Create should succeed");

    // Update with external_refs
    project.external_refs = vec!["gh:owner/repo#123".to_string()];
    repo.update(&project).await.expect("Update should succeed");

    let retrieved = repo.get("extref02").await.expect("Get should succeed");
    assert_eq!(
        retrieved.external_refs,
        vec!["gh:owner/repo#123".to_string()]
    );
}

// =============================================================================
// FTS5 SEARCH TESTS
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_project_by_title() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00001".to_string(),
        title: "Rust Backend Project".to_string(),
        description: Some("Building a backend service".to_string()),
        tags: vec!["backend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "fts00002".to_string(),
        title: "Python Data Science".to_string(),
        description: Some("ML and analytics".to_string()),
        tags: vec!["ml".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
    })
    .await
    .unwrap();

    // Search for "Rust" in title
    let results = repo
        .search("Rust", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find 1 project with 'Rust'");
    assert_eq!(results.items[0].id, "fts00001");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_project_by_description() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00003".to_string(),
        title: "Project Alpha".to_string(),
        description: Some("Authentication and authorization system".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "fts00004".to_string(),
        title: "Project Beta".to_string(),
        description: Some("Frontend UI components".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
    })
    .await
    .unwrap();

    // Search for "authentication" in description
    let results = repo
        .search("authentication", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        1,
        "Should find 1 project with 'authentication'"
    );
    assert_eq!(results.items[0].id, "fts00003");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_project_by_tags() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00005".to_string(),
        title: "Microservices".to_string(),
        description: None,
        tags: vec!["backend".to_string(), "kubernetes".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    })
    .await
    .unwrap();

    // Search for "kubernetes" in tags
    let results = repo
        .search("kubernetes", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        1,
        "Should find 1 project with 'kubernetes' tag"
    );
    assert_eq!(results.items[0].id, "fts00005");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_project_by_external_refs() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00006".to_string(),
        title: "GitHub Integration".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec!["owner/repo#123".to_string(), "JIRA-456".to_string()],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    })
    .await
    .unwrap();

    // Search for GitHub issue
    let results = repo
        .search("owner/repo#123", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        1,
        "Should find project by external_refs"
    );
    assert_eq!(results.items[0].id, "fts00006");

    // Search for JIRA ticket
    let results = repo
        .search("JIRA-456", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find project by JIRA ticket");
    assert_eq!(results.items[0].id, "fts00006");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_boolean_operators() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00007".to_string(),
        title: "Rust Backend API".to_string(),
        description: Some("RESTful service".to_string()),
        tags: vec!["rust".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "fts00008".to_string(),
        title: "Python Backend API".to_string(),
        description: Some("GraphQL service".to_string()),
        tags: vec!["python".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
    })
    .await
    .unwrap();

    // AND operator
    let results = repo
        .search("Rust AND API", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find Rust + API");
    assert_eq!(results.items[0].id, "fts00007");

    // OR operator
    let results = repo
        .search("RESTful OR GraphQL", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 2, "Should find both with OR");

    // NOT operator
    let results = repo
        .search("Backend NOT Python", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should exclude Python");
    assert_eq!(results.items[0].id, "fts00007");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_phrase_query() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00009".to_string(),
        title: "Machine Learning Platform".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "fts00010".to_string(),
        title: "Learning Management System".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
    })
    .await
    .unwrap();

    // Phrase search should match exact phrase
    let results = repo
        .search("\"Machine Learning\"", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find exact phrase");
    assert_eq!(results.items[0].id, "fts00009");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_special_characters() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00011".to_string(),
        title: "Test Project".to_string(),
        description: Some("Using {curly} [brackets] (parens)".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    })
    .await
    .unwrap();

    // Should sanitize special chars and not error
    let results = repo
        .search("{curly}", None)
        .await
        .expect("Search should not error on special chars");
    assert_eq!(results.items.len(), 1, "Should find despite special chars");
}

#[tokio::test(flavor = "multi_thread")]
async fn list_projects_with_offset_without_limit() {
    let db = setup_db().await;
    let repo = db.projects();

    // Create 3 projects
    for i in 1..=3 {
        repo.create(&Project {
            id: format!("offset{:02}", i),
            title: format!("Project {}", i),
            description: Some(format!("Description {}", i)),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some(format!("2025-01-01 00:00:{:02}", i)),
            updated_at: Some(format!("2025-01-01 00:00:{:02}", i)),
        })
        .await
        .unwrap();
    }

    // List with offset=1 but NO limit
    // SQL requires LIMIT when using OFFSET, so this should work
    let query = ProjectQuery {
        page: crate::db::PageSort {
            limit: None,
            offset: Some(1),
            sort_by: None,
            sort_order: None,
        },
        tags: None,
    };

    let result = repo
        .list(Some(&query))
        .await
        .expect("List with offset should succeed");
    assert_eq!(
        result.items.len(),
        2,
        "Should return 2 projects after skipping 1"
    );
    assert_eq!(result.total, 3, "Total should still be 3");
    assert_eq!(result.items[0].title, "Project 2");
    assert_eq!(result.items[1].title, "Project 3");
}
