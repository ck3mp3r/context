use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::commands::skill::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
use tokio::net::TcpListener;

// =============================================================================
// Integration Tests - TDD for Skills CLI
// =============================================================================

/// Spawn a test HTTP server with in-memory database
async fn spawn_test_server() -> (String, String, tokio::task::JoinHandle<()>) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    // Create test project
    let project_id = sqlx::query_scalar::<_, String>(
        "INSERT INTO project (id, title, description, tags, created_at, updated_at) 
         VALUES ('test0000', 'Test Project', 'Test project for CLI tests', '[]', datetime('now'), datetime('now')) 
         RETURNING id"
    )
    .fetch_one(db.pool())
    .await
    .expect("Failed to create test project");

    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
    );
    let app = routes::create_router(state, false);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (url, project_id, handle)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_skill_crud_operations() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // CREATE: Skill with all fields populated
    let create_result = create_skill(
        &api_client,
        CreateSkillRequest {
            name: "rust-programming".to_string(),
            description: Some("Systems programming language".to_string()),
            instructions: Some("Follow the Rust Book and practice daily".to_string()),
            tags: Some(vec![
                "rust".to_string(),
                "systems".to_string(),
                "programming".to_string(),
            ]),
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: Some(vec![project_id.clone()]),
        },
    )
    .await;
    assert!(create_result.is_ok(), "Should create skill with full data");

    // Extract skill ID
    let output = create_result.unwrap();
    let skill_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .and_then(|s| s.split(':').nth(1))
        .map(|s| s.trim())
        .expect("Failed to extract skill ID");

    // GET: Verify all fields persisted
    let get_result = get_skill(&api_client, skill_id, "json")
        .await
        .expect("Failed to get skill");
    let fetched_skill: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(fetched_skill["name"], "rust-programming");
    assert_eq!(fetched_skill["description"], "Systems programming language");
    assert_eq!(
        fetched_skill["instructions"],
        "Follow the Rust Book and practice daily"
    );
    assert_eq!(
        fetched_skill["tags"],
        json!(["rust", "systems", "programming"])
    );
    assert_eq!(fetched_skill["project_ids"], json!([project_id]));

    // UPDATE: Change multiple fields
    let update_result = update_skill(
        &api_client,
        skill_id,
        UpdateSkillRequest {
            name: Some("advanced-rust-programming".to_string()),
            description: Some("Advanced systems programming with Rust".to_string()),
            instructions: Some(
                "Focus on unsafe Rust, FFI, and performance optimization".to_string(),
            ),
            tags: Some(vec![
                "rust".to_string(),
                "advanced".to_string(),
                "systems".to_string(),
            ]),
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: None,
        },
    )
    .await;
    assert!(update_result.is_ok(), "Should update skill");

    // GET: Verify updates
    let get_updated = get_skill(&api_client, skill_id, "json")
        .await
        .expect("Failed to get updated skill");
    let updated_skill: serde_json::Value = serde_json::from_str(&get_updated).unwrap();

    assert_eq!(updated_skill["name"], "advanced-rust-programming");
    assert_eq!(
        updated_skill["description"],
        "Advanced systems programming with Rust"
    );
    assert_eq!(
        updated_skill["instructions"],
        "Focus on unsafe Rust, FFI, and performance optimization"
    );
    assert_eq!(
        updated_skill["tags"],
        json!(["rust", "advanced", "systems"])
    );

    // DELETE: Requires force flag
    let delete_no_force = delete_skill(&api_client, skill_id, false).await;
    assert!(delete_no_force.is_err(), "Should fail without --force");
    assert!(delete_no_force.unwrap_err().to_string().contains("--force"));

    // DELETE: With force flag
    let delete_result = delete_skill(&api_client, skill_id, true).await;
    assert!(delete_result.is_ok(), "Should delete skill with --force");

    // GET: Verify deletion
    let get_deleted = get_skill(&api_client, skill_id, "json").await;
    assert!(get_deleted.is_err(), "Should not find deleted skill");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_empty() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    let page = PageParams {
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = list_skills(&api_client, None, None, page, "json")
        .await
        .expect("Should list empty skills");

    let skills: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(skills.len(), 0, "Should have no skills");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_with_filters() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create multiple skills
    create_skill(
        &api_client,
        CreateSkillRequest {
            name: "rust".to_string(),
            description: Some("Systems language".to_string()),
            instructions: Some("Practice systems programming".to_string()),
            tags: Some(vec!["rust".to_string(), "systems".to_string()]),
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: Some(vec![project_id.clone()]),
        },
    )
    .await
    .expect("Failed to create skill 1");

    create_skill(
        &api_client,
        CreateSkillRequest {
            name: "python".to_string(),
            description: Some("High-level language".to_string()),
            instructions: Some("Learn Python basics".to_string()),
            tags: Some(vec!["python".to_string(), "scripting".to_string()]),
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: None,
        },
    )
    .await
    .expect("Failed to create skill 2");

    create_skill(
        &api_client,
        CreateSkillRequest {
            name: "go".to_string(),
            description: Some("Cloud native language".to_string()),
            instructions: Some("Build cloud apps".to_string()),
            tags: Some(vec!["go".to_string(), "cloud".to_string()]),
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: Some(vec![project_id.clone()]),
        },
    )
    .await
    .expect("Failed to create skill 3");

    // List all
    let page = PageParams {
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };
    let result = list_skills(&api_client, None, None, page, "json")
        .await
        .expect("Should list all skills");
    let all_skills: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(all_skills.len(), 3, "Should have 3 skills");

    // Filter by project_id
    let page = PageParams {
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };
    let result = list_skills(&api_client, Some(&project_id), None, page, "json")
        .await
        .expect("Should list filtered skills");
    let filtered: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(filtered.len(), 2, "Should have 2 skills in project");

    // Filter by tags
    let page = PageParams {
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };
    let result = list_skills(&api_client, None, Some("rust"), page, "json")
        .await
        .expect("Should list filtered skills");
    let tagged: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(tagged.len(), 1, "Should have 1 skill with rust tag");
    assert_eq!(tagged[0].name, "rust");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_pagination_and_sorting() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create skills with different names
    for name in ["alpha", "beta", "gamma", "delta"] {
        create_skill(
            &api_client,
            CreateSkillRequest {
                name: name.to_string(),
                description: Some("Test description".to_string()),
                instructions: Some("Test instructions".to_string()),
                tags: None,
                license: None,
                compatibility: None,
                allowed_tools: None,
                metadata: None,
                origin_url: None,
                origin_ref: None,
                project_ids: None,
            },
        )
        .await
        .expect("Failed to create skill");
    }

    // Test limit
    let page = PageParams {
        limit: Some(2),
        offset: None,
        sort: None,
        order: None,
    };
    let result = list_skills(&api_client, None, None, page, "json")
        .await
        .expect("Should list with limit");
    let limited: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(limited.len(), 2, "Should limit to 2 skills");

    // Test offset
    let page = PageParams {
        limit: Some(2),
        offset: Some(2),
        sort: None,
        order: None,
    };
    let result = list_skills(&api_client, None, None, page, "json")
        .await
        .expect("Should list with offset");
    let offset_skills: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(offset_skills.len(), 2, "Should have 2 skills after offset");

    // Test sorting by name
    let page = PageParams {
        limit: None,
        offset: None,
        sort: Some("name"),
        order: Some("asc"),
    };
    let result = list_skills(&api_client, None, None, page, "json")
        .await
        .expect("Should list sorted");
    let sorted: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(sorted[0].name, "alpha", "Should be sorted alphabetically");
    assert_eq!(sorted[3].name, "gamma", "Should be sorted alphabetically");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    let result = get_skill(&api_client, "nonexistent", "json").await;
    assert!(result.is_err(), "Should fail to get non-existent skill");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_skill_minimal() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Attempt to create with only name (should fail - description and instructions required)
    let result = create_skill(
        &api_client,
        CreateSkillRequest {
            name: "javascript".to_string(),
            description: None,
            instructions: None,
            tags: None,
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: None,
        },
    )
    .await;
    assert!(
        result.is_err(),
        "Should fail - description and instructions are required"
    );

    // Create with all required fields
    let result = create_skill(
        &api_client,
        CreateSkillRequest {
            name: "javascript".to_string(),
            description: Some("JavaScript programming language".to_string()),
            instructions: Some("Use for web development".to_string()),
            tags: None,
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: None,
        },
    )
    .await;
    assert!(
        result.is_ok(),
        "Should create skill with all required fields"
    );

    let output = result.unwrap();
    assert!(output.contains("javascript"), "Should mention skill name");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_partial() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create skill
    let create_result = create_skill(
        &api_client,
        CreateSkillRequest {
            name: "typescript".to_string(),
            description: Some("JavaScript superset".to_string()),
            instructions: Some("Learn gradually".to_string()),
            tags: None,
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: None,
        },
    )
    .await
    .expect("Failed to create skill");

    let skill_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .and_then(|s| s.split(':').nth(1))
        .map(|s| s.trim())
        .expect("Failed to extract skill ID");

    // Update only name
    let update_result = update_skill(
        &api_client,
        skill_id,
        UpdateSkillRequest {
            name: Some("typescript-pro".to_string()),
            description: None,
            instructions: None,
            tags: None,
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: None,
        },
    )
    .await;
    assert!(update_result.is_ok(), "Should update partial fields");

    // Verify only name changed
    let get_result = get_skill(&api_client, skill_id, "json")
        .await
        .expect("Failed to get updated skill");
    let updated: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(updated["name"], "typescript-pro");
    assert_eq!(updated["description"], "JavaScript superset"); // Unchanged
    assert_eq!(updated["instructions"], "Learn gradually"); // Unchanged
}

#[tokio::test(flavor = "multi_thread")]
async fn test_skill_table_output() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create a skill
    create_skill(
        &api_client,
        CreateSkillRequest {
            name: "docker".to_string(),
            description: Some("Containerization".to_string()),
            instructions: Some("Learn container orchestration".to_string()),
            tags: Some(vec!["docker".to_string(), "devops".to_string()]),
            license: None,
            compatibility: None,
            allowed_tools: None,
            metadata: None,
            origin_url: None,
            origin_ref: None,
            project_ids: None,
        },
    )
    .await
    .expect("Failed to create skill");

    // Get table format
    let page = PageParams {
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };
    let result = list_skills(&api_client, None, None, page, "table")
        .await
        .expect("Should list in table format");

    assert!(result.contains("docker"), "Table should contain skill name");
    assert!(result.contains("ID"), "Table should have ID header");
    assert!(result.contains("Name"), "Table should have Name header");
    assert!(result.contains("Tags"), "Table should have Tags header");
}
