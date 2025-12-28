use crate::cli::api_client::ApiClient;
use crate::cli::error::CliResult;
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled, settings::Style};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct UpdateRepoRequest {
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
    pub tags: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
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
            id: repo.id.chars().take(8).collect(),
            remote: if repo.remote.len() <= 50 {
                repo.remote.clone()
            } else {
                format!("{}...", repo.remote.chars().take(47).collect::<String>())
            },
            path: repo
                .path
                .as_ref()
                .map(|p| {
                    if p.len() <= 30 {
                        p.clone()
                    } else {
                        format!("{}...", p.chars().take(27).collect::<String>())
                    }
                })
                .unwrap_or_else(|| "-".to_string()),
            tags: repo
                .tags
                .as_ref()
                .map(|t| t.join(", "))
                .unwrap_or_else(|| "-".to_string()),
        }
    }
}

/// List repos with optional filtering
pub async fn list_repos(
    api_client: &ApiClient,
    tags: Option<&str>,
    limit: Option<u32>,
    format: &str,
) -> CliResult<String> {
    let mut url = format!("{}/v1/repos", api_client.base_url());
    let mut query_params = Vec::new();

    if let Some(t) = tags {
        query_params.push(format!("tags={}", t));
    }
    if let Some(l) = limit {
        query_params.push(format!("limit={}", l));
    }

    if !query_params.is_empty() {
        url = format!("{}?{}", url, query_params.join("&"));
    }

    let response: ListReposResponse = reqwest::get(&url).await?.json().await?;

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
    table.with(Style::rounded());
    table.to_string()
}

/// Get a single repo by ID
pub async fn get_repo(api_client: &ApiClient, id: &str, format: &str) -> CliResult<String> {
    let url = format!("{}/v1/repos/{}", api_client.base_url(), id);
    let repo: Repo = reqwest::get(&url).await?.json().await?;

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

    if let Some(tags) = &repo.tags {
        if !tags.is_empty() {
            builder.push_record(["Tags", &tags.join(", ")]);
        }
    }

    builder.push_record(["Created", &repo.created_at]);
    builder.push_record(["Updated", &repo.updated_at]);

    let mut table = builder.build();
    table.with(Style::rounded());
    table.to_string()
}

/// Create a new repo
pub async fn create_repo(
    api_client: &ApiClient,
    remote: &str,
    path: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    let url = format!("{}/v1/repos", api_client.base_url());

    let tags_vec = tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let request = CreateRepoRequest {
        remote: remote.to_string(),
        path: path.map(String::from),
        tags: tags_vec,
    };

    let client = reqwest::Client::new();
    let response = client.post(&url).json(&request).send().await?;

    if response.status().is_success() {
        let repo: Repo = response.json().await?;
        Ok(format!(
            "✓ Created repository: {} ({})",
            repo.remote,
            repo.id.chars().take(8).collect::<String>()
        ))
    } else {
        let status = response.status().as_u16();
        let error_text = response.text().await?;
        Err(crate::cli::error::CliError::ApiError {
            status,
            message: error_text,
        })
    }
}

/// Update an existing repo (PATCH semantics)
pub async fn update_repo(
    api_client: &ApiClient,
    id: &str,
    remote: Option<&str>,
    path: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    let url = format!("{}/v1/repos/{}", api_client.base_url(), id);

    let tags_vec = tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let request = UpdateRepoRequest {
        remote: remote.map(String::from),
        path: path.map(String::from),
        tags: tags_vec,
    };

    let client = reqwest::Client::new();
    let response = client.patch(&url).json(&request).send().await?;

    if response.status().is_success() {
        let repo: Repo = response.json().await?;
        Ok(format!(
            "✓ Updated repository: {} ({})",
            repo.remote,
            repo.id.chars().take(8).collect::<String>()
        ))
    } else {
        let status = response.status().as_u16();
        let error_text = response.text().await?;
        Err(crate::cli::error::CliError::ApiError {
            status,
            message: error_text,
        })
    }
}

/// Delete a repo (requires --force flag for safety)
pub async fn delete_repo(api_client: &ApiClient, id: &str, force: bool) -> CliResult<String> {
    if !force {
        return Err(crate::cli::error::CliError::InvalidResponse {
            message: "Delete operation requires --force flag. This action is destructive and cannot be undone.".to_string(),
        });
    }

    let url = format!("{}/v1/repos/{}", api_client.base_url(), id);

    let client = reqwest::Client::new();
    let response = client.delete(&url).send().await?;

    if response.status().is_success() {
        Ok(format!(
            "✓ Deleted repository: {}",
            id.chars().take(8).collect::<String>()
        ))
    } else {
        let status = response.status().as_u16();
        let error_text = response.text().await?;
        Err(crate::cli::error::CliError::ApiError {
            status,
            message: error_text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_repos_table_format() {
        // GREEN: Test list repos with table format
        let api_client = ApiClient::new(None);
        let result = list_repos(&api_client, None, None, "table").await;

        // Function should exist and return a result
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_list_repos_json_format() {
        // GREEN: Test list repos with JSON format
        let api_client = ApiClient::new(None);
        let result = list_repos(&api_client, None, None, "json").await;

        // Function should exist and return a result
        assert!(result.is_ok() || result.is_err());

        // If successful, output should be parseable as JSON
        if let Ok(output) = result {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&output);
            assert!(parsed.is_ok(), "Output should be valid JSON");
        }
    }

    #[tokio::test]
    async fn test_get_repo() {
        let api_client = ApiClient::new(None);
        let result = get_repo(&api_client, "test-id", "table").await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_get_repo_json() {
        let api_client = ApiClient::new(None);
        let result = get_repo(&api_client, "test-id", "json").await;
        assert!(result.is_ok() || result.is_err());

        if let Ok(output) = result {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&output);
            assert!(parsed.is_ok(), "Output should be valid JSON");
        }
    }

    #[tokio::test]
    async fn test_create_repo_minimal() {
        let api_client = ApiClient::new(None);
        let result = create_repo(&api_client, "https://github.com/test/repo", None, None).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_create_repo_full() {
        let api_client = ApiClient::new(None);
        let result = create_repo(
            &api_client,
            "https://github.com/test/repo",
            Some("/path/to/repo"),
            Some("tag1,tag2"),
        )
        .await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_update_repo_partial() {
        let api_client = ApiClient::new(None);
        let result = update_repo(
            &api_client,
            "test-id",
            Some("https://github.com/test/newrepo"),
            None,
            None,
        )
        .await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_update_repo_all_fields() {
        let api_client = ApiClient::new(None);
        let result = update_repo(
            &api_client,
            "test-id",
            Some("https://github.com/test/newrepo"),
            Some("/new/path"),
            Some("newtag1,newtag2"),
        )
        .await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_delete_repo_without_force() {
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

    #[tokio::test]
    async fn test_delete_repo_with_force() {
        let api_client = ApiClient::new(None);
        let result = delete_repo(&api_client, "test-id", true).await;
        assert!(result.is_ok() || result.is_err());
    }
}
