use crate::cli::api_client::ApiClient;
use crate::cli::error::CliResult;
use crate::cli::utils::{apply_table_style, format_tags, parse_tags, truncate_with_ellipsis};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};

#[derive(Debug, Serialize, Deserialize)]
struct ListProjectsResponse {
    items: Vec<Project>,
    total: usize,
    limit: usize,
    offset: usize,
}

#[derive(Debug, Serialize)]
struct CreateProjectRequest {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct UpdateProjectRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
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
            id: project.id.clone(),
            title: truncate_with_ellipsis(&project.title, 40),
            description: project
                .description
                .as_ref()
                .map(|d| truncate_with_ellipsis(d, 50))
                .unwrap_or_else(|| "-".to_string()),
            tags: format_tags(project.tags.as_ref()),
        }
    }
}

/// List projects with optional filtering
pub async fn list_projects(
    api_client: &ApiClient,
    tags: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get("/api/v1/projects");

    if let Some(t) = tags {
        request = request.query(&[("tags", t)]);
    }
    if let Some(l) = limit {
        request = request.query(&[("limit", l.to_string())]);
    }
    if let Some(o) = offset {
        request = request.query(&[("offset", o.to_string())]);
    }

    let response: ListProjectsResponse = request.send().await?.json().await?;

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
    apply_table_style(&mut table);
    table.to_string()
}

/// Get a single project by ID
pub async fn get_project(api_client: &ApiClient, id: &str, format: &str) -> CliResult<String> {
    let project: Project = api_client
        .get(&format!("/api/v1/projects/{}", id))
        .send()
        .await?
        .json()
        .await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&project)?),
        _ => Ok(format_project_detail(&project)),
    }
}

fn format_project_detail(project: &Project) -> String {
    use tabled::builder::Builder;

    let mut builder = Builder::default();

    builder.push_record(["Project ID", &project.id]);
    builder.push_record(["Title", &project.title]);

    if let Some(desc) = &project.description {
        builder.push_record(["Description", desc]);
    }

    if let Some(tags) = &project.tags
        && !tags.is_empty()
    {
        builder.push_record(["Tags", &tags.join(", ")]);
    }

    builder.push_record(["Created", &project.created_at]);
    builder.push_record(["Updated", &project.updated_at]);

    let mut table = builder.build();
    apply_table_style(&mut table);
    table.to_string()
}

/// Create a new project
pub async fn create_project(
    api_client: &ApiClient,
    title: &str,
    description: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    let request = CreateProjectRequest {
        title: title.to_string(),
        description: description.map(String::from),
        tags: parse_tags(tags),
    };

    let response = api_client
        .post("/api/v1/projects")
        .json(&request)
        .send()
        .await?;

    let project: Project = ApiClient::handle_response(response).await?;
    Ok(format!(
        "✓ Created project: {} ({})",
        project.title, project.id
    ))
}

/// Update an existing project (PATCH semantics - only updates provided fields)
pub async fn update_project(
    api_client: &ApiClient,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    let request = UpdateProjectRequest {
        title: title.map(String::from),
        description: description.map(String::from),
        tags: parse_tags(tags),
    };

    let response = api_client
        .patch(&format!("/api/v1/projects/{}", id))
        .json(&request)
        .send()
        .await?;

    let project: Project = ApiClient::handle_response(response).await?;
    Ok(format!(
        "✓ Updated project: {} ({})",
        project.title, project.id
    ))
}

/// Delete a project (requires --force flag for safety)
pub async fn delete_project(api_client: &ApiClient, id: &str, force: bool) -> CliResult<String> {
    // Safety check: require --force flag
    if !force {
        return Err(crate::cli::error::CliError::InvalidResponse {
            message: "Delete operation requires --force flag. This action is destructive and cannot be undone.".to_string(),
        });
    }

    let response = api_client
        .delete(&format!("/api/v1/projects/{}", id))
        .send()
        .await?;

    // For delete, we expect no body on success, so we don't use handle_response
    // Just check status
    if response.status().is_success() {
        Ok(format!("✓ Deleted project: {}", id))
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
