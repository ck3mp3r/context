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
    #[serde(default)]
    project_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PatchRepoRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_ids: Option<Vec<String>>,
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
    project_ids: Option<&str>,
) -> CliResult<String> {
    let request = CreateRepoRequest {
        remote: remote.to_string(),
        path: path.map(String::from),
        tags: parse_tags(tags).unwrap_or_default(),
        project_ids: parse_tags(project_ids).unwrap_or_default(),
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
    project_ids: Option<&str>,
) -> CliResult<String> {
    let request = PatchRepoRequest {
        remote: remote.map(String::from),
        path: path.map(String::from),
        tags: parse_tags(tags),
        project_ids: parse_tags(project_ids),
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
