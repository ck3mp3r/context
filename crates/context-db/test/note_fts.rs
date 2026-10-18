//! Tests for SqliteNoteRepository.

use crate::SqliteDatabase;
use context_core::{Database, HasNotes, Note, NoteQuery, NoteRepository};

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
        deleted_at: None,
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
