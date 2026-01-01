use crate::cli::api_client::ApiClient;
use crate::cli::error::CliResult;
use crate::cli::utils::{apply_table_style, format_tags, parse_tags, truncate_with_ellipsis};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};

#[derive(Debug, Serialize, Deserialize)]
struct ListReposResponse {
    items: Vec<Repo>,
    total: usize,
    limit: usize,
    offset: usize,
}

#[derive(Debug, Serialize)]
struct CreateRepoRequest {
    remote: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PatchRepoRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Repo {
    pub id: String,
    pub remote: String,
    pub path: Option<String>,
    pub tags: Vec<String>,
    pub project_ids: Vec<String>,
    pub created_at: String,
}

#[derive(Tabled)]
struct RepoDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Remote")]
    remote: String,
    #[tabled(rename = "Path")]
    path: String,
    #[tabled(rename = "Tags")]
    tags: String,
}

impl From<&Repo> for RepoDisplay {
    fn from(repo: &Repo) -> Self {
        Self {
            id: repo.id.clone(),
            remote: truncate_with_ellipsis(&repo.remote, 50),
            path: repo
                .path
                .as_ref()
                .map(|p| truncate_with_ellipsis(p, 30))
                .unwrap_or_else(|| "-".to_string()),
            tags: format_tags(Some(&repo.tags)),
        }
    }
}

/// List repos with optional filtering
pub async fn list_repos(
    api_client: &ApiClient,
    tags: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get("/api/v1/repos");

    if let Some(t) = tags {
        request = request.query(&[("tags", t)]);
    }
    if let Some(l) = limit {
        request = request.query(&[("limit", l.to_string())]);
    }
    if let Some(o) = offset {
        request = request.query(&[("offset", o.to_string())]);
    }

    let response: ListReposResponse = request.send().await?.json().await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&response.items)?),
        _ => Ok(format_table(&response.items)),
    }
}

fn format_table(repos: &[Repo]) -> String {
    if repos.is_empty() {
        return "No repositories found.".to_string();
    }

    let display_repos: Vec<RepoDisplay> = repos.iter().map(|r| r.into()).collect();
    let mut table = Table::new(display_repos);
    apply_table_style(&mut table);
    table.to_string()
}

/// Get a single repo by ID
pub async fn get_repo(api_client: &ApiClient, id: &str, format: &str) -> CliResult<String> {
    let repo: Repo = api_client
        .get(&format!("/api/v1/repos/{}", id))
        .send()
        .await?
        .json()
        .await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&repo)?),
        _ => Ok(format_repo_detail(&repo)),
    }
}

fn format_repo_detail(repo: &Repo) -> String {
    use tabled::builder::Builder;

    let mut builder = Builder::default();

    builder.push_record(["Repo ID", &repo.id]);
    builder.push_record(["Remote", &repo.remote]);

    if let Some(path) = &repo.path {
        builder.push_record(["Path", path]);
    }

    if !repo.tags.is_empty() {
        builder.push_record(["Tags", &repo.tags.join(", ")]);
    }

    if !repo.project_ids.is_empty() {
        builder.push_record(["Projects", &repo.project_ids.join(", ")]);
    }

    builder.push_record(["Created", &repo.created_at]);

    let mut table = builder.build();
    apply_table_style(&mut table);
    table.to_string()
}

/// Create a new repo
pub async fn create_repo(
    api_client: &ApiClient,
    remote: &str,
    path: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    let request = CreateRepoRequest {
        remote: remote.to_string(),
        path: path.map(String::from),
        tags: parse_tags(tags).unwrap_or_default(),
    };

    let response = api_client
        .post("/api/v1/repos")
        .json(&request)
        .send()
        .await?;

    let repo: Repo = ApiClient::handle_response(response).await?;
    Ok(format!(
        "✓ Created repository: {} ({})",
        repo.remote, repo.id
    ))
}

/// Update an existing repo (PATCH semantics)
pub async fn update_repo(
    api_client: &ApiClient,
    id: &str,
    remote: Option<&str>,
    path: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    let request = PatchRepoRequest {
        remote: remote.map(String::from),
        path: path.map(String::from),
        tags: parse_tags(tags),
    };

    let response = api_client
        .patch(&format!("/api/v1/repos/{}", id))
        .json(&request)
        .send()
        .await?;

    let repo: Repo = ApiClient::handle_response(response).await?;
    Ok(format!(
        "✓ Updated repository: {} ({})",
        repo.remote, repo.id
    ))
}

/// Delete a repo (requires --force flag for safety)
pub async fn delete_repo(api_client: &ApiClient, id: &str, force: bool) -> CliResult<String> {
    if !force {
        return Err(crate::cli::error::CliError::InvalidResponse {
            message: "Delete operation requires --force flag. This action is destructive and cannot be undone.".to_string(),
        });
    }

    let response = api_client
        .delete(&format!("/api/v1/repos/{}", id))
        .send()
        .await?;

    if response.status().is_success() {
        Ok(format!("✓ Deleted repository: {}", id))
    } else {
        let status = response.status().as_u16();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        Err(crate::cli::error::CliError::ApiError {
            status,
            message: error_text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AppState, routes};
    use crate::db::{Database, SqliteDatabase};
    use crate::sync::MockGitOps;
    use tokio::net::TcpListener;

    /// Spawn a test HTTP server with in-memory database
    async fn spawn_test_server() -> (String, tokio::task::JoinHandle<()>) {
        let db = SqliteDatabase::in_memory()
            .await
            .expect("Failed to create test database");
        db.migrate().expect("Failed to run migrations");
        let state = AppState::new(db, crate::sync::SyncManager::new(MockGitOps::new()));
        let app = routes::create_router(state, false);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        (url, handle)
    }

    #[tokio::test]
    async fn test_delete_repo_without_force() {
        // Test the --force flag validation (pure logic, no HTTP needed)
        let api_client = ApiClient::new(None);
        let result = delete_repo(&api_client, "test-id", false).await;

        assert!(result.is_err());
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("--force"),
                "Error should mention --force flag"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_repos() {
        let (url, _handle) = spawn_test_server().await;
        let api_client = ApiClient::new(Some(url));

        let result = list_repos(&api_client, None, None, None, "json").await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 0); // Initially empty
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_create_and_get_repo() {
        let (url, _handle) = spawn_test_server().await;
        let api_client = ApiClient::new(Some(url));

        // Create
        let create_result =
            create_repo(&api_client, "https://github.com/test/repo", None, None).await;
        assert!(create_result.is_ok());

        let output = create_result.unwrap();
        assert!(output.contains("Created repository"));

        // Extract ID from output (contains ID in message)
        // For now just verify list shows the repo
        let list_result = list_repos(&api_client, None, None, None, "json").await;
        assert!(list_result.is_ok());

        let output = list_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 1);
    }
}
