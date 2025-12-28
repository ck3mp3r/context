use crate::cli::api_client::ApiClient;
use crate::cli::error::CliResult;
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled, settings::Style};

#[derive(Debug, Serialize, Deserialize)]
struct ListProjectsResponse {
    items: Vec<Project>,
    total: usize,
    limit: usize,
    offset: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Tabled)]
struct ProjectDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Description")]
    description: String,
    #[tabled(rename = "Tags")]
    tags: String,
}

impl From<&Project> for ProjectDisplay {
    fn from(project: &Project) -> Self {
        Self {
            id: project.id.chars().take(8).collect(),
            title: if project.title.len() <= 40 {
                project.title.clone()
            } else {
                format!("{}...", project.title.chars().take(37).collect::<String>())
            },
            description: project
                .description
                .as_ref()
                .map(|d| {
                    if d.len() <= 50 {
                        d.clone()
                    } else {
                        format!("{}...", d.chars().take(47).collect::<String>())
                    }
                })
                .unwrap_or_else(|| "-".to_string()),
            tags: project
                .tags
                .as_ref()
                .map(|t| t.join(", "))
                .unwrap_or_else(|| "-".to_string()),
        }
    }
}

/// List projects with optional filtering
pub async fn list_projects(
    api_client: &ApiClient,
    tags: Option<&str>,
    limit: Option<u32>,
    format: &str,
) -> CliResult<String> {
    let mut url = format!("{}/v1/projects", api_client.base_url());
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

    let response: ListProjectsResponse = reqwest::get(&url).await?.json().await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&response.items)?),
        _ => Ok(format_table(&response.items)),
    }
}

fn format_table(projects: &[Project]) -> String {
    if projects.is_empty() {
        return "No projects found.".to_string();
    }

    let display_projects: Vec<ProjectDisplay> = projects.iter().map(|p| p.into()).collect();
    let mut table = Table::new(display_projects);
    table.with(Style::rounded());
    table.to_string()
}

/// Get a single project by ID (stub for now)
pub async fn get_project(_api_client: &ApiClient, _id: &str, _format: &str) -> CliResult<String> {
    todo!("Implement get_project in TDD cycle")
}

/// Create a new project (stub for now)
pub async fn create_project(
    _api_client: &ApiClient,
    _title: &str,
    _description: Option<&str>,
    _tags: Option<&str>,
) -> CliResult<String> {
    todo!("Implement create_project in TDD cycle")
}

/// Update an existing project (stub for now)
pub async fn update_project(
    _api_client: &ApiClient,
    _id: &str,
    _title: Option<&str>,
    _description: Option<&str>,
    _tags: Option<&str>,
) -> CliResult<String> {
    todo!("Implement update_project in TDD cycle")
}

/// Delete a project (stub for now)
pub async fn delete_project(_api_client: &ApiClient, _id: &str, _force: bool) -> CliResult<String> {
    todo!("Implement delete_project in TDD cycle")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_projects_table_format() {
        // This should return a formatted table with projects
        let api_client = ApiClient::new(None);
        let result = list_projects(&api_client, None, None, "table").await;

        // We expect it to succeed (not test actual API, just function exists)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_projects_json_format() {
        // This should return JSON formatted projects
        let api_client = ApiClient::new(None);
        let result = list_projects(&api_client, None, None, "json").await;

        // Should return valid result
        assert!(result.is_ok());

        // If successful, output should be parseable as JSON
        if let Ok(output) = result {
            // Should be valid JSON (array of projects)
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&output);
            assert!(parsed.is_ok(), "Output should be valid JSON");
        }
    }
}
