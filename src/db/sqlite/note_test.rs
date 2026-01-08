//! Tests for SqliteNoteRepository.

use crate::db::{Database, Note, NoteQuery, NoteRepository, PageSort, SortOrder, SqliteDatabase};

fn generate_id() -> String {
    use crate::db::utils::generate_entity_id;
    generate_entity_id()
}

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in_memory database");
    db.migrate().expect("Migration should succeed");
    db
}

fn make_note(id: &str, title: &str, content: &str) -> Note {
    Note {
        id: id.to_string(),
        title: title.to_string(),
        content: content.to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],    // Empty by default - relationships managed separately
        project_ids: vec![], // Empty by default - relationships managed separately
        subnote_count: None, // Computed field - not set on creation
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    }
}

// =============================================================================
// FTS5 Tag Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_notes_by_tag_content() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes with specific tags
    let mut note1 = make_note("fts00001", "Rust Programming", "Learning async/await");
    note1.tags = vec!["rust".to_string(), "programming".to_string()];
    notes.create(&note1).await.unwrap();

    let mut note2 = make_note("fts00002", "Python Guide", "Flask tutorial");
    note2.tags = vec!["python".to_string(), "web".to_string()];
    notes.create(&note2).await.unwrap();

    let mut note3 = make_note("fts00003", "JavaScript Basics", "ES6 features");
    note3.tags = vec!["javascript".to_string(), "programming".to_string()];
    notes.create(&note3).await.unwrap();

    // Search for "rust" - should find the note with "rust" tag using FTS5
    let results = notes
        .search("rust", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find note with 'rust' tag");
    assert_eq!(results.items[0].id, "fts00001");

    // Search for "programming" - should find 2 notes with "programming" tag
    let results = notes
        .search("programming", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        2,
        "Should find both notes with 'programming' tag"
    );

    // Search for "python" - should find note with "python" tag
    let results = notes
        .search("python", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find note with 'python' tag");
    assert_eq!(results.items[0].id, "fts00002");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_combines_tag_and_content_results() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes where search term appears in different fields
    let mut note1 = make_note("fts00004", "Database Design", "PostgreSQL patterns");
    note1.tags = vec!["database".to_string(), "backend".to_string()];
    notes.create(&note1).await.unwrap();

    let mut note2 = make_note("fts00005", "API Testing", "Testing database connections");
    note2.tags = vec!["testing".to_string(), "api".to_string()];
    notes.create(&note2).await.unwrap();

    let mut note3 = make_note("fts00006", "Frontend State", "Redux store patterns");
    note3.tags = vec!["frontend".to_string()];
    notes.create(&note3).await.unwrap();

    // Search for "database" - should find note1 (tag) AND note2 (content)
    let results = notes
        .search("database", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        2,
        "Should find notes where 'database' appears in tag OR content"
    );

    let found_ids: Vec<&str> = results.items.iter().map(|n| n.id.as_str()).collect();
    assert!(
        found_ids.contains(&"fts00004"),
        "Should find note with 'database' tag"
    );
    assert!(
        found_ids.contains(&"fts00005"),
        "Should find note with 'database' in content"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_supports_boolean_operators_with_tags() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes with different tag combinations
    let mut note1 = make_note("fts00007", "Rust Web Development", "Axum framework guide");
    note1.tags = vec!["rust".to_string(), "web".to_string()];
    notes.create(&note1).await.unwrap();

    let mut note2 = make_note("fts00008", "Python Web Development", "Django tutorial");
    note2.tags = vec!["python".to_string(), "web".to_string()];
    notes.create(&note2).await.unwrap();

    let mut note3 = make_note("fts00009", "Rust CLI Tools", "Command-line parsing");
    note3.tags = vec!["rust".to_string(), "cli".to_string()];
    notes.create(&note3).await.unwrap();

    // FTS5 AND operator: search for notes with both "rust" AND "web"
    let results = notes
        .search("rust AND web", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        1,
        "Should find only note with both 'rust' AND 'web' tags"
    );
    assert_eq!(results.items[0].id, "fts00007");

    // FTS5 OR operator: search for notes with "python" OR "cli"
    let results = notes
        .search("python OR cli", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        2,
        "Should find notes with 'python' OR 'cli' tags"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_with_tag_filter_and_tag_search() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes to test filtering and searching
    let mut note1 = make_note("fts00010", "API Design", "REST patterns");
    note1.tags = vec![
        "api".to_string(),
        "backend".to_string(),
        "architecture".to_string(),
    ];
    notes.create(&note1).await.unwrap();

    let mut note2 = make_note("fts00011", "Backend Architecture", "Microservices");
    note2.tags = vec!["backend".to_string(), "architecture".to_string()];
    notes.create(&note2).await.unwrap();

    let mut note3 = make_note("fts00012", "Frontend Architecture", "Component design");
    note3.tags = vec!["frontend".to_string(), "architecture".to_string()];
    notes.create(&note3).await.unwrap();

    // Search for "architecture" with "backend" tag filter
    // Should find notes where "architecture" is in content/tags AND has "backend" tag
    let query = NoteQuery {
        tags: Some(vec!["backend".to_string()]),
        ..Default::default()
    };
    let results = notes
        .search("architecture", Some(&query))
        .await
        .expect("Search should succeed");

    assert_eq!(
        results.items.len(),
        2,
        "Should find notes with 'architecture' in tags/content that also have 'backend' tag"
    );

    let found_ids: Vec<&str> = results.items.iter().map(|n| n.id.as_str()).collect();
    assert!(found_ids.contains(&"fts00010"));
    assert!(found_ids.contains(&"fts00011"));
    assert!(
        !found_ids.contains(&"fts00012"),
        "Should not find frontend note"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_phrase_query_in_tags() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes with multi-word tags
    let mut note1 = make_note("fts00013", "Project Planning", "Sprint organization");
    note1.tags = vec!["project-management".to_string(), "agile".to_string()];
    notes.create(&note1).await.unwrap();

    let mut note2 = make_note("fts00014", "Code Review", "Best practices");
    note2.tags = vec!["code-quality".to_string(), "review".to_string()];
    notes.create(&note2).await.unwrap();

    // Search for exact phrase or term matching
    let results = notes
        .search("project", None)
        .await
        .expect("Search should succeed");

    // Should find the note because "project" matches "project-management" tag
    assert!(
        results.items.iter().any(|n| n.id == "fts00013"),
        "Should find note with tag containing 'project'"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_unbalanced_quotes() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create test notes
    notes
        .create(&make_note("quotest1", "Test Note", "Some content here"))
        .await
        .unwrap();

    // Search with unbalanced quote should not crash
    let result = notes.search("\"test", None).await;
    assert!(
        result.is_ok(),
        "Search with unbalanced quote should not crash"
    );

    // Search with balanced quotes should work
    let result = notes.search("\"test\"", None).await;
    assert!(result.is_ok(), "Search with balanced quotes should work");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_curly_braces() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create test notes
    notes
        .create(&make_note(
            "brace001",
            "Rust Programming",
            "Using async and await",
        ))
        .await
        .unwrap();

    // Search with curly braces should sanitize to "rust" and find the note
    let result = notes.search("{rust}", None).await;
    assert!(result.is_ok(), "Search with curly braces should not crash");

    let items = result.unwrap().items;
    assert!(
        items.iter().any(|n| n.id == "brace001"),
        "Should find note after sanitizing curly braces"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_square_brackets() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create test notes
    notes
        .create(&make_note(
            "brack001",
            "Async Runtime",
            "tokio and async-std",
        ))
        .await
        .unwrap();

    // Search with square brackets should sanitize to "tokio"
    let result = notes.search("[tokio]", None).await;
    assert!(
        result.is_ok(),
        "Search with square brackets should not crash"
    );

    let items = result.unwrap().items;
    assert!(
        items.iter().any(|n| n.id == "brack001"),
        "Should find note after sanitizing square brackets"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_parentheses() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create test notes
    notes
        .create(&make_note(
            "paren001",
            "Async Programming",
            "async/await patterns",
        ))
        .await
        .unwrap();

    // Search with parentheses should sanitize to "async"
    let result = notes.search("(async)", None).await;
    assert!(result.is_ok(), "Search with parentheses should not crash");

    let items = result.unwrap().items;
    assert!(
        items.iter().any(|n| n.id == "paren001"),
        "Should find note after sanitizing parentheses"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_angle_brackets() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create test notes
    notes
        .create(&make_note(
            "angle001",
            "Generic Types",
            "Using generic types in Rust",
        ))
        .await
        .unwrap();

    // Search with angle brackets should sanitize to "generic"
    let result = notes.search("<generic>", None).await;
    assert!(
        result.is_ok(),
        "Search with angle brackets should not crash"
    );

    let items = result.unwrap().items;
    assert!(
        items.iter().any(|n| n.id == "angle001"),
        "Should find note after sanitizing angle brackets"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_mixed_special_chars() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create test notes
    notes
        .create(&make_note(
            "mixed001",
            "Web Development",
            "Using rust for async web frameworks with tokio",
        ))
        .await
        .unwrap();

    // Search with multiple special characters should sanitize and find all terms
    let result = notes.search("rust{async}[tokio]<web>", None).await;
    assert!(
        result.is_ok(),
        "Search with mixed special chars should not crash"
    );

    let items = result.unwrap().items;
    assert!(
        items.iter().any(|n| n.id == "mixed001"),
        "Should find note after sanitizing all special characters"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_boolean_with_special_chars() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create test notes
    notes
        .create(&make_note("bool0001", "Rust Guide", "rust programming"))
        .await
        .unwrap();
    notes
        .create(&make_note("bool0002", "Tokio Guide", "tokio runtime"))
        .await
        .unwrap();
    notes
        .create(&make_note(
            "bool0003",
            "Complete Guide",
            "rust and tokio together",
        ))
        .await
        .unwrap();

    // Boolean query with special characters should preserve AND operator
    let result = notes.search("rust AND {tokio}", None).await;
    assert!(
        result.is_ok(),
        "Boolean query with special chars should not crash"
    );

    let items = result.unwrap().items;
    assert!(
        items.iter().any(|n| n.id == "bool0003"),
        "Should find note matching both terms with AND operator"
    );
    assert!(
        !items.iter().any(|n| n.id == "bool0001"),
        "Should not find note with only rust"
    );
    assert!(
        !items.iter().any(|n| n.id == "bool0002"),
        "Should not find note with only tokio"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_phrase_with_special_chars() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create test notes
    notes
        .create(&make_note(
            "phrase01",
            "Framework Guide",
            "A web framework for rust",
        ))
        .await
        .unwrap();

    // Phrase search with special characters should work
    let result = notes.search("\"{web} framework\"", None).await;
    assert!(
        result.is_ok(),
        "Phrase search with special chars should not crash"
    );

    let items = result.unwrap().items;
    assert!(
        items.iter().any(|n| n.id == "phrase01"),
        "Should find note with phrase after sanitizing special chars"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_empty_string() {
    let db = setup_db().await;
    let notes = db.notes();

    // Empty search should return empty result set, not crash
    let result = notes.search("", None).await;
    assert!(result.is_ok(), "Empty search should not crash");

    let items = result.unwrap().items;
    assert!(items.is_empty(), "Empty search should return empty results");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_only_special_chars() {
    let db = setup_db().await;
    let notes = db.notes();

    // Search with only special characters should return empty result
    let result = notes.search("{[]}", None).await;
    assert!(
        result.is_ok(),
        "Search with only special chars should not crash"
    );

    let items = result.unwrap().items;
    assert!(
        items.is_empty(),
        "Search with only special chars should return empty results"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_whitespace_only() {
    let db = setup_db().await;
    let notes = db.notes();

    // Whitespace-only search should return empty result
    let result = notes.search("   ", None).await;
    assert!(result.is_ok(), "Whitespace-only search should not crash");

    let items = result.unwrap().items;
    assert!(
        items.is_empty(),
        "Whitespace-only search should return empty results"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_create_and_get() {
    let db = setup_db().await;
    let notes = db.notes();

    let note = Note {
        id: "note0001".to_string(),
        title: "My First Note".to_string(),
        content: "This is markdown content\n\n## Heading\n\nWith paragraphs.".to_string(),
        tags: vec!["session".to_string(), "important".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec![],    // Empty by default - relationships managed separately
        project_ids: vec![], // Empty by default - relationships managed separately
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    notes.create(&note).await.expect("Create should succeed");

    let retrieved = notes.get("note0001").await.expect("Get should succeed");
    assert_eq!(retrieved.id, note.id);
    assert_eq!(retrieved.title, note.title);
    assert_eq!(retrieved.content, note.content);
    assert_eq!(retrieved.tags, note.tags);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_get_nonexistent_returns_not_found() {
    let db = setup_db().await;
    let notes = db.notes();

    let result = notes.get("nonexist").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list() {
    let db = setup_db().await;
    let notes = db.notes();

    // Initially empty
    let result = notes.list(None).await.expect("List should succeed");
    assert!(result.items.is_empty());

    // Add notes
    notes
        .create(&make_note("noteaaa1", "First", "Content one"))
        .await
        .unwrap();
    notes
        .create(&make_note("notebbb2", "Second", "Content two"))
        .await
        .unwrap();

    let result = notes.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_update() {
    let db = setup_db().await;
    let notes = db.notes();

    let mut note = make_note("noteupd1", "Original Title", "Original content");
    notes.create(&note).await.expect("Create should succeed");

    note.title = "Updated Title".to_string();
    note.content = "Updated content with more text".to_string();
    note.tags = vec!["updated".to_string()];
    notes.update(&note).await.expect("Update should succeed");

    let retrieved = notes.get("noteupd1").await.expect("Get should succeed");
    assert_eq!(retrieved.title, "Updated Title");
    assert_eq!(retrieved.content, "Updated content with more text");
    assert_eq!(retrieved.tags, vec!["updated".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_delete() {
    let db = setup_db().await;
    let notes = db.notes();

    let note = make_note("notedel1", "To Delete", "Will be deleted");
    notes.create(&note).await.expect("Create should succeed");

    notes
        .delete("notedel1")
        .await
        .expect("Delete should succeed");

    let result = notes.get("notedel1").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_search() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes with specific content
    notes
        .create(&make_note(
            "notesrc1",
            "API Design",
            "REST endpoints for user management",
        ))
        .await
        .unwrap();
    notes
        .create(&make_note(
            "notesrc2",
            "Database Schema",
            "SQLite tables for user data",
        ))
        .await
        .unwrap();
    notes
        .create(&make_note(
            "notesrc3",
            "Frontend Guide",
            "React components for dashboard",
        ))
        .await
        .unwrap();

    // Search for "user" - should find 2 notes
    let results = notes
        .search("user", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 2);

    // Search for "React" - should find 1 note
    let results = notes
        .search("React", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].title, "Frontend Guide");

    // Search for nonexistent term
    let results = notes
        .search("kubernetes", None)
        .await
        .expect("Search should succeed");
    assert!(results.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list_with_tag_filter() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes with different tags
    let mut note1 = make_note("notetag1", "Rust Guide", "About Rust");
    note1.tags = vec!["rust".to_string(), "programming".to_string()];
    notes.create(&note1).await.unwrap();

    let mut note2 = make_note("notetag2", "Python Guide", "About Python");
    note2.tags = vec!["python".to_string(), "programming".to_string()];
    notes.create(&note2).await.unwrap();

    let mut note3 = make_note("notetag3", "Cooking Recipe", "About cooking");
    note3.tags = vec!["cooking".to_string()];
    notes.create(&note3).await.unwrap();

    // Filter by "rust" tag - should find 1
    let query = NoteQuery {
        tags: Some(vec!["rust".to_string()]),
        ..Default::default()
    };
    let results = notes.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].title, "Rust Guide");

    // Filter by "programming" tag - should find 2
    let query = NoteQuery {
        tags: Some(vec!["programming".to_string()]),
        ..Default::default()
    };
    let results = notes.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(results.items.len(), 2);
    assert_eq!(results.total, 2);

    // Filter by nonexistent tag
    let query = NoteQuery {
        tags: Some(vec!["nonexistent".to_string()]),
        ..Default::default()
    };
    let results = notes.list(Some(&query)).await.expect("List should succeed");
    assert!(results.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_search_with_tag_filter() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes with different tags and content
    let mut note1 = make_note("srctag01", "API Design", "REST API patterns");
    note1.tags = vec!["api".to_string(), "backend".to_string()];
    notes.create(&note1).await.unwrap();

    let mut note2 = make_note("srctag02", "API Testing", "Testing API endpoints");
    note2.tags = vec!["api".to_string(), "testing".to_string()];
    notes.create(&note2).await.unwrap();

    let mut note3 = make_note("srctag03", "Frontend APIs", "Calling APIs from React");
    note3.tags = vec!["frontend".to_string()];
    notes.create(&note3).await.unwrap();

    // Search for "API" with no tag filter - should find all 3
    let results = notes
        .search("API", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 3);

    // Search for "API" with "backend" tag filter - should find 1
    let query = NoteQuery {
        tags: Some(vec!["backend".to_string()]),
        ..Default::default()
    };
    let results = notes
        .search("API", Some(&query))
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].title, "API Design");

    // Search for "API" with "api" tag filter - should find 2
    let query = NoteQuery {
        tags: Some(vec!["api".to_string()]),
        ..Default::default()
    };
    let results = notes
        .search("API", Some(&query))
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 2);
    assert_eq!(results.total, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_get_loads_repo_and_project_relationships() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create repos first (for foreign key constraints)
    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("repo0001")
        .bind("https://github.com/test/repo1")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert repo1 should succeed");

    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("repo0002")
        .bind("https://github.com/test/repo2")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert repo2 should succeed");

    // Create project first (for foreign key constraints)
    sqlx::query("INSERT INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("proj0001")
        .bind("Test Project")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert project should succeed");

    // Create a note
    let note = make_note("reltest1", "Test Note", "Content");
    notes.create(&note).await.expect("Create should succeed");

    // Insert relationships into junction tables
    sqlx::query("INSERT INTO note_repo (note_id, repo_id) VALUES (?, ?)")
        .bind("reltest1")
        .bind("repo0001")
        .execute(db.pool())
        .await
        .expect("Insert note_repo should succeed");

    sqlx::query("INSERT INTO note_repo (note_id, repo_id) VALUES (?, ?)")
        .bind("reltest1")
        .bind("repo0002")
        .execute(db.pool())
        .await
        .expect("Insert note_repo should succeed");

    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("proj0001")
        .bind("reltest1")
        .execute(db.pool())
        .await
        .expect("Insert project_note should succeed");

    // Get note and verify relationships are loaded
    let retrieved = notes.get("reltest1").await.expect("Get should succeed");

    assert_eq!(
        retrieved.repo_ids.len(),
        2,
        "Should load 2 repo relationships"
    );
    assert!(retrieved.repo_ids.contains(&"repo0001".to_string()));
    assert!(retrieved.repo_ids.contains(&"repo0002".to_string()));

    assert_eq!(
        retrieved.project_ids.len(),
        1,
        "Should load 1 project relationship"
    );
    assert!(retrieved.project_ids.contains(&"proj0001".to_string()));
}

// =============================================================================
// Size Validation Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn note_create_with_warn_size_content_succeeds_with_warning() {
    use crate::db::models::NOTE_WARN_SIZE;

    let db = setup_db().await;
    let notes = db.notes();

    // Create note just over warning threshold
    let large_content = "x".repeat(NOTE_WARN_SIZE + 100);
    let note = Note {
        id: "warn0001".to_string(),
        title: "Large Note".to_string(),
        content: large_content.clone(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    // Should succeed (warn size is advisory, not enforced)
    let result = notes.create(&note).await;
    assert!(
        result.is_ok(),
        "Note over WARN_SIZE should succeed (warning is advisory)"
    );

    let retrieved = notes.get("warn0001").await.expect("Should retrieve note");
    assert_eq!(retrieved.content.len(), large_content.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_create_at_hard_max_succeeds() {
    use crate::db::models::NOTE_HARD_MAX;

    let db = setup_db().await;
    let notes = db.notes();

    // Create note exactly at hard maximum
    let max_content = "x".repeat(NOTE_HARD_MAX);
    let note = Note {
        id: "max00001".to_string(),
        title: "Maximum Size Note".to_string(),
        content: max_content.clone(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    let result = notes.create(&note).await;
    assert!(result.is_ok(), "Note exactly at HARD_MAX should succeed");
}

#[tokio::test(flavor = "multi_thread")]
async fn note_create_over_hard_max_fails() {
    use crate::db::models::NOTE_HARD_MAX;

    let db = setup_db().await;
    let notes = db.notes();

    // Create note over hard maximum
    let oversized_content = "x".repeat(NOTE_HARD_MAX + 1);
    let note = Note {
        id: "over0001".to_string(),
        title: "Oversized Note".to_string(),
        content: oversized_content,
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    let result = notes.create(&note).await;
    assert!(result.is_err(), "Note over HARD_MAX should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("exceeds maximum size") || err_msg.contains("too large"),
        "Error message should mention size limit, got: {}",
        err_msg
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_update_over_hard_max_fails() {
    use crate::db::models::NOTE_HARD_MAX;

    let db = setup_db().await;
    let notes = db.notes();

    // Create normal note
    let note = make_note("upd00001", "Normal Note", "Initial content");
    notes.create(&note).await.expect("Create should succeed");

    // Try to update with oversized content
    let oversized_content = "x".repeat(NOTE_HARD_MAX + 1);
    let mut updated_note = note.clone();
    updated_note.content = oversized_content;

    let result = notes.update(&updated_note).await;
    assert!(result.is_err(), "Update over HARD_MAX should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("exceeds maximum size") || err_msg.contains("too large"),
        "Error message should mention size limit, got: {}",
        err_msg
    );
}

// =============================================================================
// Metadata-Only Retrieval Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn note_get_with_content_excluded() {
    let db = setup_db().await;
    let notes = db.notes();

    let note = make_note(
        "meta0001",
        "Test Note",
        "This is the content that should be excluded",
    );
    notes.create(&note).await.expect("Create should succeed");

    // Get note without content
    let retrieved = notes
        .get_metadata_only("meta0001")
        .await
        .expect("Get should succeed");

    assert_eq!(retrieved.id, "meta0001");
    assert_eq!(retrieved.title, "Test Note");
    assert_eq!(
        retrieved.content, "",
        "Content should be empty when excluded"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list_with_content_excluded() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create multiple notes
    let note1 = make_note("list0001", "Note 1", "Content 1 - should be excluded");
    let note2 = make_note("list0002", "Note 2", "Content 2 - should be excluded");
    notes.create(&note1).await.expect("Create should succeed");
    notes.create(&note2).await.expect("Create should succeed");

    // List notes without content
    let result = notes
        .list_metadata_only(None)
        .await
        .expect("List should succeed");

    assert_eq!(result.items.len(), 2);
    for item in &result.items {
        assert_eq!(item.content, "", "Content should be empty for all items");
        assert!(!item.title.is_empty(), "Title should still be present");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_hyphens_in_boolean_queries() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create test notes with hyphenated terms
    notes
        .create(&make_note(
            "hyphen01",
            "CLI Tools",
            "Documentation about command-line interfaces",
        ))
        .await
        .unwrap();

    notes
        .create(&make_note(
            "hyphen02",
            "Real-time Systems",
            "Guide to real-time programming",
        ))
        .await
        .unwrap();

    notes
        .create(&make_note(
            "hyphen03",
            "API Design",
            "Best practices for REST APIs",
        ))
        .await
        .unwrap();

    // Test 1: Boolean query with hyphen should NOT crash
    let result = notes.search("CLI OR command-line", None).await;
    assert!(
        result.is_ok(),
        "Search with 'CLI OR command-line' should not crash"
    );

    // Test 2: Hyphenated term alone should work
    let result = notes.search("command-line", None).await;
    assert!(
        result.is_ok(),
        "Search with 'command-line' should not crash"
    );

    // Test 3: Complex Boolean with hyphens should work
    let result = notes.search("real-time AND programming", None).await;
    assert!(
        result.is_ok(),
        "Search with 'real-time AND programming' should not crash"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_timestamps_are_optional() {
    let db = setup_db().await;

    // Test 1: Provided timestamps are respected
    let note_with_timestamps = Note {
        id: String::new(),
        title: "Note with timestamps".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-15 10:00:00".to_string()),
        updated_at: Some("2025-01-15 11:00:00".to_string()),
    };

    let created_with_ts = db
        .notes()
        .create(&note_with_timestamps)
        .await
        .expect("Create note");
    assert_eq!(
        created_with_ts.created_at,
        Some("2025-01-15 10:00:00".to_string())
    );
    assert_eq!(
        created_with_ts.updated_at,
        Some("2025-01-15 11:00:00".to_string())
    );

    // Test 2: None timestamps are auto-generated
    let note_without_timestamps = Note {
        id: String::new(),
        title: "Note without timestamps".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };

    let created_without_ts = db
        .notes()
        .create(&note_without_timestamps)
        .await
        .expect("Create note");
    assert!(created_without_ts.created_at.is_some());
    assert!(created_without_ts.updated_at.is_some());
    assert!(!created_without_ts.created_at.as_ref().unwrap().is_empty());
    assert!(!created_without_ts.updated_at.as_ref().unwrap().is_empty());
}

// =============================================================================
// Hierarchical Notes (parent_id and idx)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_note_with_parent_id() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent Note".to_string(),
        content: "Parent content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create child note
    let child = Note {
        id: generate_id(),
        title: "Child Note".to_string(),
        content: "Child content".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_child = db.notes().create(&child).await.unwrap();

    // Verify parent_id is set
    assert_eq!(created_child.parent_id, Some(created_parent.id));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_subnote_with_idx() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent Note".to_string(),
        content: "Parent content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create child note with idx
    let child = Note {
        id: generate_id(),
        title: "Child Note".to_string(),
        content: "Child content".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: Some(10),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_child = db.notes().create(&child).await.unwrap();

    // Verify idx is set
    assert_eq!(created_child.idx, Some(10));
    assert_eq!(created_child.parent_id, Some(created_parent.id));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_subnotes_ordered_by_idx() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent Note".to_string(),
        content: "Parent content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create multiple child notes with different idx values
    let child1 = Note {
        id: generate_id(),
        title: "Child 1 (idx=30)".to_string(),
        content: "Should be third".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: Some(30),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Child 2 (idx=10)".to_string(),
        content: "Should be first".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: Some(10),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    let child3 = Note {
        id: generate_id(),
        title: "Child 3 (idx=20)".to_string(),
        content: "Should be second".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: Some(20),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child3).await.unwrap();

    // Query for subnotes filtered by parent_id, sorted by idx
    let query = NoteQuery {
        parent_id: Some(created_parent.id.clone()),
        page: PageSort {
            sort_by: Some("idx".to_string()),
            sort_order: Some(SortOrder::Asc),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();

    // Verify ordering by idx
    assert_eq!(result.items.len(), 3);
    assert_eq!(result.items[0].title, "Child 2 (idx=10)");
    assert_eq!(result.items[1].title, "Child 3 (idx=20)");
    assert_eq!(result.items[2].title, "Child 1 (idx=30)");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_note_type_filter_returns_only_parent_notes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 2 parent notes
    let parent1 = Note {
        id: generate_id(),
        title: "Parent 1".to_string(),
        content: "Parent content 1".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent1 = db.notes().create(&parent1).await.unwrap();

    let parent2 = Note {
        id: generate_id(),
        title: "Parent 2".to_string(),
        content: "Parent content 2".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&parent2).await.unwrap();

    // Create 2 subnotes
    let child1 = Note {
        id: generate_id(),
        title: "Child 1".to_string(),
        content: "Child content 1".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Child 2".to_string(),
        content: "Child content 2".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    // Test filter for parent notes only (note_type="note")
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 parent notes");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert!(
            note.parent_id.is_none(),
            "All notes should have parent_id = None"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_note_type_filter_returns_only_subnotes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 1 parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent".to_string(),
        content: "Parent content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create 2 subnotes
    let child1 = Note {
        id: generate_id(),
        title: "Child 1".to_string(),
        content: "Child content 1".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Child 2".to_string(),
        content: "Child content 2".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    // Test filter for subnotes only (note_type="subnote")
    let query = NoteQuery {
        note_type: Some("subnote".to_string()),
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 subnotes");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert!(
            note.parent_id.is_some(),
            "All notes should have parent_id != None"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_with_note_type_filter_returns_only_parent_notes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 2 parent notes with searchable content
    let parent1 = Note {
        id: generate_id(),
        title: "Parent Note 1".to_string(),
        content: "Searchable content in parent note".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent1 = db.notes().create(&parent1).await.unwrap();

    let parent2 = Note {
        id: generate_id(),
        title: "Parent Note 2".to_string(),
        content: "More searchable content here".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&parent2).await.unwrap();

    // Create 2 subnotes with searchable content
    let child1 = Note {
        id: generate_id(),
        title: "Subnote 1".to_string(),
        content: "Searchable content in subnote".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Subnote 2".to_string(),
        content: "Another searchable subnote".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    // Search with note_type=note filter (should return only parent notes)
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };

    let result = db.notes().search("searchable", Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 parent notes");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert!(
            note.parent_id.is_none(),
            "All notes should have parent_id = None"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_with_note_type_filter_returns_only_subnotes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 1 parent note with searchable content
    let parent = Note {
        id: generate_id(),
        title: "Parent Note".to_string(),
        content: "Searchable content in parent".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create 2 subnotes with searchable content
    let child1 = Note {
        id: generate_id(),
        title: "Subnote 1".to_string(),
        content: "Searchable content in subnote".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Subnote 2".to_string(),
        content: "Another searchable subnote".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    // Search with note_type=subnote filter (should return only subnotes)
    let query = NoteQuery {
        note_type: Some("subnote".to_string()),
        ..Default::default()
    };

    let result = db.notes().search("searchable", Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 subnotes");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert!(
            note.parent_id.is_some(),
            "All notes should have parent_id != None"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_with_parent_id_filter() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 2 parent notes
    let parent1 = Note {
        id: generate_id(),
        title: "Parent 1".to_string(),
        content: "Parent 1 content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent1 = db.notes().create(&parent1).await.unwrap();

    let parent2 = Note {
        id: generate_id(),
        title: "Parent 2".to_string(),
        content: "Parent 2 content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent2 = db.notes().create(&parent2).await.unwrap();

    // Create subnotes for parent1 with searchable content
    let child1_1 = Note {
        id: generate_id(),
        title: "Child 1.1".to_string(),
        content: "Findme in parent1 child".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child1_1).await.unwrap();

    let child1_2 = Note {
        id: generate_id(),
        title: "Child 1.2".to_string(),
        content: "Findme in another parent1 child".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child1_2).await.unwrap();

    // Create subnote for parent2 with searchable content
    let child2_1 = Note {
        id: generate_id(),
        title: "Child 2.1".to_string(),
        content: "Findme in parent2 child".to_string(),
        tags: vec![],
        parent_id: Some(created_parent2.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&child2_1).await.unwrap();

    // Search with parent_id filter (should return only subnotes of parent1)
    let query = NoteQuery {
        parent_id: Some(created_parent1.id.clone()),
        ..Default::default()
    };

    let result = db.notes().search("Findme", Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 subnotes of parent1");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert_eq!(
            note.parent_id.as_ref().unwrap(),
            &created_parent1.id,
            "All notes should have parent_id = parent1.id"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_parent_notes_sorted_by_last_activity() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 3 parent notes
    // We rely on actual timestamps since triggers override passed timestamps
    let parent1 = Note {
        id: generate_id(),
        title: "Parent 1".to_string(),
        content: "First parent (oldest)".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let _created_parent1 = db.notes().create(&parent1).await.unwrap();

    // Wait to ensure timestamp difference (SQLite has second precision by default)
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let parent2 = Note {
        id: generate_id(),
        title: "Parent 2".to_string(),
        content: "Second parent".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent2 = db.notes().create(&parent2).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let parent3 = Note {
        id: generate_id(),
        title: "Parent 3".to_string(),
        content: "Third parent".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent3 = db.notes().create(&parent3).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Create a subnote for parent2 - will be updated to have most recent activity
    let subnote = Note {
        id: generate_id(),
        title: "Subnote of Parent 2".to_string(),
        content: "Medium activity".to_string(),
        tags: vec![],
        parent_id: Some(created_parent2.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_subnote = db.notes().create(&subnote).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update parent3 directly - should have second most recent activity
    let mut updated_parent3 = created_parent3.clone();
    updated_parent3.content = "Updated parent 3".to_string();
    db.notes().update(&updated_parent3).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update the subnote - parent2 should now have the most recent activity
    let mut updated_subnote = created_subnote.clone();
    updated_subnote.content = "Latest activity".to_string();
    db.notes().update(&updated_subnote).await.unwrap();

    // Query parent notes with type=note filter (no explicit sort)
    // Expected order by last_activity_at DESC:
    // 1. Parent 2 (most recent - subnote updated last)
    // 2. Parent 3 (medium - updated directly)
    // 3. Parent 1 (oldest - no updates)
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 3, "Should return all 3 parent notes");
    assert_eq!(result.items.len(), 3);

    // Verify sort order by last activity (DESC)
    assert_eq!(
        result.items[0].title, "Parent 2",
        "Parent 2 should be first (most recent activity via subnote)"
    );
    assert_eq!(
        result.items[1].title, "Parent 3",
        "Parent 3 should be second (updated directly)"
    );
    assert_eq!(
        result.items[2].title, "Parent 1",
        "Parent 1 should be third (oldest, no updates)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_parent_notes_explicit_sort_overrides_activity_sort() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 2 parent notes
    let parent_a = Note {
        id: generate_id(),
        title: "Z Parent".to_string(),
        content: "Should be last alphabetically".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created_parent_a = db.notes().create(&parent_a).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let parent_b = Note {
        id: generate_id(),
        title: "A Parent".to_string(),
        content: "Should be first alphabetically".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&parent_b).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Create subnote for Z Parent - makes it most recently active
    let subnote = Note {
        id: generate_id(),
        title: "Subnote".to_string(),
        content: "Latest activity".to_string(),
        tags: vec![],
        parent_id: Some(created_parent_a.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&subnote).await.unwrap();

    // Query with explicit sort by title ASC
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        page: crate::db::PageSort {
            sort_by: Some("title".to_string()),
            sort_order: Some(crate::db::SortOrder::Asc),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return 2 parent notes");
    assert_eq!(result.items.len(), 2);

    // Verify sort order is by title ASC, NOT by activity
    assert_eq!(
        result.items[0].title, "A Parent",
        "Should be first alphabetically despite older activity"
    );
    assert_eq!(
        result.items[1].title, "Z Parent",
        "Should be last alphabetically despite recent activity"
    );
}

// =============================================================================
// Subnote Count Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_parent_notes_include_subnote_count() {
    let db = setup_db().await;

    // Create parent note with 3 subnotes
    let parent_with_subnotes = Note {
        id: generate_id(),
        title: "Parent with children".to_string(),
        content: "Has subnotes".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&parent_with_subnotes).await.unwrap();

    // Create 3 subnotes
    for i in 1..=3 {
        let subnote = Note {
            id: generate_id(),
            title: format!("Subnote {}", i),
            content: format!("Content {}", i),
            tags: vec![],
            parent_id: Some(parent_with_subnotes.id.clone()),
            idx: Some(i),
            repo_ids: vec![],
            project_ids: vec![],
            subnote_count: None,
            created_at: None,
            updated_at: None,
        };
        db.notes().create(&subnote).await.unwrap();
    }

    // Create parent note without subnotes
    let parent_no_subnotes = Note {
        id: generate_id(),
        title: "Parent alone".to_string(),
        content: "No subnotes".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&parent_no_subnotes).await.unwrap();

    // Query parent notes with type=note
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return 2 parent notes");

    // Find each parent in results
    let parent_with = result
        .items
        .iter()
        .find(|n| n.id == parent_with_subnotes.id)
        .expect("Parent with subnotes should be in results");

    let parent_without = result
        .items
        .iter()
        .find(|n| n.id == parent_no_subnotes.id)
        .expect("Parent without subnotes should be in results");

    // Verify subnote_count is populated correctly
    assert_eq!(
        parent_with.subnote_count,
        Some(3),
        "Parent with 3 subnotes should have subnote_count = 3"
    );
    assert_eq!(
        parent_without.subnote_count,
        Some(0),
        "Parent with no subnotes should have subnote_count = 0"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_subnote_count_only_for_parent_notes() {
    let db = setup_db().await;

    // Create parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent".to_string(),
        content: "Content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&parent).await.unwrap();

    // Create subnote
    let subnote = Note {
        id: generate_id(),
        title: "Subnote".to_string(),
        content: "Content".to_string(),
        tags: vec![],
        parent_id: Some(parent.id.clone()),
        idx: Some(1),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&subnote).await.unwrap();

    // Query WITHOUT type filter - subnote_count should be None
    let result = db.notes().list(None).await.unwrap();
    for note in &result.items {
        assert_eq!(
            note.subnote_count, None,
            "subnote_count should be None when not filtering by type=note"
        );
    }

    // Query WITH type=note - subnote_count should be Some
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };
    let result = db.notes().list(Some(&query)).await.unwrap();
    let parent_result = result.items.iter().find(|n| n.id == parent.id).unwrap();
    assert_eq!(
        parent_result.subnote_count,
        Some(1),
        "subnote_count should be Some(1) when filtering by type=note"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_metadata_only_includes_subnote_count() {
    let db = setup_db().await;

    // Create parent with 2 subnotes
    let parent = Note {
        id: generate_id(),
        title: "Parent".to_string(),
        content: "This is the full content that should NOT be in metadata_only".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&parent).await.unwrap();

    for i in 1..=2 {
        let subnote = Note {
            id: generate_id(),
            title: format!("Subnote {}", i),
            content: format!("Content {}", i),
            tags: vec![],
            parent_id: Some(parent.id.clone()),
            idx: Some(i),
            repo_ids: vec![],
            project_ids: vec![],
            subnote_count: None,
            created_at: None,
            updated_at: None,
        };
        db.notes().create(&subnote).await.unwrap();
    }

    // Query with list_metadata_only and type=note
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };
    let result = db.notes().list_metadata_only(Some(&query)).await.unwrap();

    let parent_result = result.items.iter().find(|n| n.id == parent.id).unwrap();

    // Verify content is empty (metadata only)
    assert_eq!(
        parent_result.content, "",
        "Content should be empty in metadata_only"
    );

    // Verify subnote_count is still populated
    assert_eq!(
        parent_result.subnote_count,
        Some(2),
        "subnote_count should be Some(2) even in metadata_only"
    );
}
