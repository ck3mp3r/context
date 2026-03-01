use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::commands::skill::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
use tempfile::TempDir;
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

    let temp_dir = TempDir::new().unwrap();

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
        temp_dir.path().join("skills"),
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

async fn spawn_test_server_with_temp_dir() -> (String, String, tokio::task::JoinHandle<()>, TempDir)
{
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    let temp_dir = TempDir::new().unwrap();

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
        temp_dir.path().join("skills"),
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

    (url, project_id, handle, temp_dir)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_skill_crud_operations() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // IMPORT: Skill from fixture
    let import_result = import_skill(
        &api_client,
        "tests/fixtures/skills/rust",
        None,
        Some(vec![project_id.clone()]),
        None,
        false,
    )
    .await;
    assert!(import_result.is_ok(), "Should import skill from fixture");

    // Extract skill ID
    let output = import_result.unwrap();
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

    assert_eq!(fetched_skill["name"], "rust");
    assert_eq!(fetched_skill["description"], "Systems programming language");
    assert!(
        fetched_skill["content"]
            .as_str()
            .unwrap()
            .contains("Follow the Rust Book")
    );
    assert_eq!(fetched_skill["project_ids"], json!([project_id]));

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

    let filter = ListSkillsFilter {
        query: None,
        project_id: None,
        tags: None,
        page: PageParams {
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
    };

    let result = list_skills(&api_client, filter, "json")
        .await
        .expect("Should list empty skills");

    let skills: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(skills.len(), 0, "Should have no skills");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_with_filters() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Import multiple skills with tags
    import_skill(
        &api_client,
        "tests/fixtures/skills/rust",
        None,
        Some(vec![project_id.clone()]),
        Some(vec!["rust".to_string()]),
        false,
    )
    .await
    .expect("Failed to import skill 1");

    import_skill(
        &api_client,
        "tests/fixtures/skills/python",
        None,
        None,
        Some(vec!["python".to_string()]),
        false,
    )
    .await
    .expect("Failed to import skill 2");

    import_skill(
        &api_client,
        "tests/fixtures/skills/go",
        None,
        Some(vec![project_id.clone()]),
        Some(vec!["go".to_string()]),
        false,
    )
    .await
    .expect("Failed to import skill 3");

    // List all
    let filter = ListSkillsFilter {
        query: None,
        project_id: None,
        tags: None,
        page: PageParams {
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
    };
    let result = list_skills(&api_client, filter, "json")
        .await
        .expect("Should list all skills");
    let all_skills: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(all_skills.len(), 3, "Should have 3 skills");

    // Filter by project_id
    let filter = ListSkillsFilter {
        query: None,
        project_id: Some(&project_id),
        tags: None,
        page: PageParams {
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
    };
    let result = list_skills(&api_client, filter, "json")
        .await
        .expect("Should list filtered skills");
    let filtered: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(filtered.len(), 2, "Should have 2 skills in project");

    // Filter by tags
    let filter = ListSkillsFilter {
        query: None,
        project_id: None,
        tags: Some("rust"),
        page: PageParams {
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
    };
    let result = list_skills(&api_client, filter, "json")
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
    // Import skills with different names for pagination/sorting tests
    for name in ["alpha", "beta", "gamma", "delta"] {
        import_skill(
            &api_client,
            &format!("tests/fixtures/skills/{}", name),
            None,
            None,
            None,
            false,
        )
        .await
        .unwrap_or_else(|_| panic!("Failed to import {} skill", name));
    }

    // Test limit
    let filter = ListSkillsFilter {
        query: None,
        project_id: None,
        tags: None,
        page: PageParams {
            limit: Some(2),
            offset: None,
            sort: None,
            order: None,
        },
    };
    let result = list_skills(&api_client, filter, "json")
        .await
        .expect("Should list with limit");
    let limited: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(limited.len(), 2, "Should limit to 2 skills");

    // Test offset
    let filter = ListSkillsFilter {
        query: None,
        project_id: None,
        tags: None,
        page: PageParams {
            limit: Some(2),
            offset: Some(2),
            sort: None,
            order: None,
        },
    };
    let result = list_skills(&api_client, filter, "json")
        .await
        .expect("Should list with offset");
    let offset_skills: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(offset_skills.len(), 2, "Should have 2 skills after offset");

    // Test sorting by name
    let filter = ListSkillsFilter {
        query: None,
        project_id: None,
        tags: None,
        page: PageParams {
            limit: None,
            offset: None,
            sort: Some("name"),
            order: Some("asc"),
        },
    };
    let result = list_skills(&api_client, filter, "json")
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
async fn test_skill_table_output() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Import a skill
    import_skill(
        &api_client,
        "tests/fixtures/skills/docker",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import skill");

    // Get table format
    let filter = ListSkillsFilter {
        query: None,
        project_id: None,
        tags: None,
        page: PageParams {
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
    };
    let result = list_skills(&api_client, filter, "table")
        .await
        .expect("Should list in table format");

    assert!(result.contains("docker"), "Table should contain skill name");
    assert!(result.contains("ID"), "Table should have ID header");
    assert!(result.contains("Name"), "Table should have Name header");
    assert!(result.contains("Tags"), "Table should have Tags header");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_metadata() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Import a skill without tags or projects
    let import_result = import_skill(
        &api_client,
        "tests/fixtures/skills/docker",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import skill");

    // Extract skill ID from import result
    let skill_id = import_result
        .split("ID: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .trim();

    // Update tags
    let _update_result = update_skill(
        &api_client,
        skill_id,
        Some(vec!["tag1".to_string(), "tag2".to_string()]),
        None,
    )
    .await
    .expect("Failed to update tags");

    // Verify tags were updated
    let skill_json = get_skill(&api_client, skill_id, "json")
        .await
        .expect("Failed to get skill");
    let skill: Skill = serde_json::from_str(&skill_json).unwrap();
    assert_eq!(skill.tags, vec!["tag1", "tag2"]);
    assert!(
        skill.project_ids.is_empty(),
        "Projects should still be empty"
    );

    // Update projects
    let _update_result = update_skill(&api_client, skill_id, None, Some(vec![project_id.clone()]))
        .await
        .expect("Failed to update projects");

    // Verify projects were updated (tags should still be there)
    let skill_json = get_skill(&api_client, skill_id, "json")
        .await
        .expect("Failed to get skill");
    let skill: Skill = serde_json::from_str(&skill_json).unwrap();
    assert_eq!(skill.tags, vec!["tag1", "tag2"], "Tags should be preserved");
    assert_eq!(skill.project_ids, vec![project_id]);

    // Update both
    let _update_result = update_skill(
        &api_client,
        skill_id,
        Some(vec!["newtag".to_string()]),
        Some(vec![]),
    )
    .await
    .expect("Failed to update both");

    // Verify both were updated
    let skill_json = get_skill(&api_client, skill_id, "json")
        .await
        .expect("Failed to get skill");
    let skill: Skill = serde_json::from_str(&skill_json).unwrap();
    assert_eq!(skill.tags, vec!["newtag"]);
    assert!(skill.project_ids.is_empty(), "Projects should be cleared");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_with_query_search() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Import skills with different content
    import_skill(
        &api_client,
        "tests/fixtures/skills/rust",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import rust skill");

    import_skill(
        &api_client,
        "tests/fixtures/skills/python",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import python skill");

    import_skill(
        &api_client,
        "tests/fixtures/skills/docker",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import docker skill");

    // Search for "systems" - should only match rust
    let filter = ListSkillsFilter {
        query: Some("systems"),
        project_id: None,
        tags: None,
        page: PageParams {
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
    };
    let result = list_skills(&api_client, filter, "json")
        .await
        .expect("Should search skills");
    let searched: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(searched.len(), 1, "Should find 1 skill matching 'systems'");
    assert_eq!(searched[0].name, "rust");

    // Search for "container" - should match docker
    let filter = ListSkillsFilter {
        query: Some("container"),
        project_id: None,
        tags: None,
        page: PageParams {
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
    };
    let result = list_skills(&api_client, filter, "json")
        .await
        .expect("Should search skills");
    let searched: Vec<Skill> = serde_json::from_str(&result).unwrap();
    assert_eq!(
        searched.len(),
        1,
        "Should find 1 skill matching 'container'"
    );
    assert_eq!(searched[0].name, "docker");

    // Search with query AND tag filter
    let filter = ListSkillsFilter {
        query: Some("programming"),
        project_id: None,
        tags: Some("rust"),
        page: PageParams {
            limit: None,
            offset: None,
            sort: None,
            order: None,
        },
    };
    let result = list_skills(&api_client, filter, "json")
        .await
        .expect("Should search with filters");
    let searched: Vec<Skill> = serde_json::from_str(&result).unwrap();
    // Should only find rust (has "systems programming" + rust tag)
    assert!(
        searched.len() <= 1,
        "Should find at most 1 skill with both query and tag"
    );
}

// =============================================================================
// Enable/Disable Cache Management Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_enable_skill_by_id() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Import a skill
    let import_result = import_skill(
        &api_client,
        "tests/fixtures/skills/rust",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import skill");

    // Extract skill ID
    let skill_id = import_result
        .split("ID: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .trim();

    // Enable the skill
    let result = enable_skill(&api_client, skill_id)
        .await
        .expect("Failed to enable skill");

    assert!(result.contains("rust"), "Should mention skill name");
    assert!(result.contains("enabled"), "Should confirm enabled");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_enable_skill_by_name() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Import a skill
    import_skill(
        &api_client,
        "tests/fixtures/skills/docker",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import skill");

    // Enable by name
    let result = enable_skill(&api_client, "docker")
        .await
        .expect("Failed to enable skill by name");

    assert!(result.contains("docker"), "Should mention skill name");
    assert!(result.contains("enabled"), "Should confirm enabled");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_enable_skill_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    let result = enable_skill(&api_client, "nonexistent").await;
    assert!(result.is_err(), "Should fail for nonexistent skill");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_disable_skill_by_id() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Import a skill
    let import_result = import_skill(
        &api_client,
        "tests/fixtures/skills/python",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import skill");

    // Extract skill ID
    let skill_id = import_result
        .split("ID: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .trim();

    // Disable the skill
    let result = disable_skill(&api_client, skill_id)
        .await
        .expect("Failed to disable skill");

    assert!(result.contains("python"), "Should mention skill name");
    assert!(result.contains("disabled"), "Should confirm disabled");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_disable_skill_by_name() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Import a skill
    import_skill(
        &api_client,
        "tests/fixtures/skills/rust",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import skill");

    // Disable by name
    let result = disable_skill(&api_client, "rust")
        .await
        .expect("Failed to disable skill by name");

    assert!(result.contains("rust"), "Should mention skill name");
    assert!(result.contains("disabled"), "Should confirm disabled");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_disable_skill_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    let result = disable_skill(&api_client, "nonexistent").await;
    assert!(result.is_err(), "Should fail for nonexistent skill");
}

// =============================================================================
// Auto-enable on Import Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_import_auto_enables_skill() {
    use std::fs;

    let (url, _project_id, _handle, temp_dir) = spawn_test_server_with_temp_dir().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Import a skill - should auto-enable and create cache
    let import_result = import_skill(
        &api_client,
        "tests/fixtures/skills/rust",
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to import skill");

    assert!(
        import_result.contains("rust"),
        "Import should succeed: {}",
        import_result
    );

    // Verify cache directory was created (using temp directory instead of /tmp)
    let cache_dir = temp_dir.path().join("skills/rust");
    assert!(
        cache_dir.exists(),
        "Cache directory should be created automatically: {}",
        cache_dir.display()
    );

    // Verify SKILL.md exists in cache
    let skill_md = cache_dir.join("SKILL.md");
    assert!(
        skill_md.exists(),
        "SKILL.md should be extracted to cache: {}",
        skill_md.display()
    );

    // Verify content is correct
    let content = fs::read_to_string(&skill_md).expect("Failed to read SKILL.md");
    assert!(
        content.contains("Follow the Rust Book"),
        "SKILL.md should contain expected content"
    );

    // TempDir will be automatically cleaned up when it goes out of scope
}
