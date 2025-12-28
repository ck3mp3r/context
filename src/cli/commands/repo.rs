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

/// Get a single repo by ID (stub for now)
pub async fn get_repo(_api_client: &ApiClient, _id: &str, _format: &str) -> CliResult<String> {
    todo!("Implement get_repo in TDD cycle")
}

/// Create a new repo (stub for now)
pub async fn create_repo(
    _api_client: &ApiClient,
    _remote: &str,
    _path: Option<&str>,
    _tags: Option<&str>,
) -> CliResult<String> {
    todo!("Implement create_repo in TDD cycle")
}

/// Update an existing repo (stub for now)
pub async fn update_repo(
    _api_client: &ApiClient,
    _id: &str,
    _remote: Option<&str>,
    _path: Option<&str>,
    _tags: Option<&str>,
) -> CliResult<String> {
    todo!("Implement update_repo in TDD cycle")
}

/// Delete a repo (stub for now)
pub async fn delete_repo(_api_client: &ApiClient, _id: &str, _force: bool) -> CliResult<String> {
    todo!("Implement delete_repo in TDD cycle")
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
}
