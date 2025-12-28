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
    offset: Option<u32>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get("/v1/projects");

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
    table.with(Style::rounded());
    table.to_string()
}

/// Get a single project by ID
pub async fn get_project(api_client: &ApiClient, id: &str, format: &str) -> CliResult<String> {
    let project: Project = api_client
        .get(&format!("/v1/projects/{}", id))
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

    if let Some(tags) = &project.tags {
        if !tags.is_empty() {
            builder.push_record(["Tags", &tags.join(", ")]);
        }
    }

    builder.push_record(["Created", &project.created_at]);
    builder.push_record(["Updated", &project.updated_at]);

    let mut table = builder.build();
    table.with(Style::rounded());
    table.to_string()
}

/// Create a new project
pub async fn create_project(
    api_client: &ApiClient,
    title: &str,
    description: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    // Parse tags from comma-separated string
    let tags_vec = tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let request = CreateProjectRequest {
        title: title.to_string(),
        description: description.map(String::from),
        tags: tags_vec,
    };

    let response = api_client
        .post("/v1/projects")
        .json(&request)
        .send()
        .await?;

    if response.status().is_success() {
        let project: Project = response.json().await?;
        Ok(format!(
            "✓ Created project: {} ({})",
            project.title,
            project.id.chars().take(8).collect::<String>()
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

/// Update an existing project (PATCH semantics - only updates provided fields)
pub async fn update_project(
    api_client: &ApiClient,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    // Parse tags from comma-separated string if provided
    let tags_vec = tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let request = UpdateProjectRequest {
        title: title.map(String::from),
        description: description.map(String::from),
        tags: tags_vec,
    };

    let response = api_client
        .patch(&format!("/v1/projects/{}", id))
        .json(&request)
        .send()
        .await?;

    if response.status().is_success() {
        let project: Project = response.json().await?;
        Ok(format!(
            "✓ Updated project: {} ({})",
            project.title,
            project.id.chars().take(8).collect::<String>()
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

/// Delete a project (requires --force flag for safety)
pub async fn delete_project(api_client: &ApiClient, id: &str, force: bool) -> CliResult<String> {
    // Safety check: require --force flag
    if !force {
        return Err(crate::cli::error::CliError::InvalidResponse {
            message: "Delete operation requires --force flag. This action is destructive and cannot be undone.".to_string(),
        });
    }

    let response = api_client
        .delete(&format!("/v1/projects/{}", id))
        .send()
        .await?;

    if response.status().is_success() {
        Ok(format!(
            "✓ Deleted project: {}",
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
    async fn test_delete_project_without_force() {
        // Test that delete without --force flag is rejected (pure logic, no HTTP needed)
        let api_client = ApiClient::new(None);
        let result = delete_project(&api_client, "test-id", false).await;

        // Should return an error about requiring --force
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
    async fn test_list_projects() {
        let (url, _handle) = spawn_test_server().await;
        let api_client = ApiClient::new(Some(url));

        let result = list_projects(&api_client, None, None, None, "json").await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_array(), "Output should be an array");
        // Migrations seed a "Default" project, so we expect 1 not 0
        assert_eq!(parsed.as_array().unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_create_and_get_project() {
        let (url, _handle) = spawn_test_server().await;
        let api_client = ApiClient::new(Some(url));

        // Create
        let create_result = create_project(
            &api_client,
            "Test Project",
            Some("Test desc"),
            Some("tag1,tag2"),
        )
        .await;
        assert!(create_result.is_ok());

        let output = create_result.unwrap();
        assert!(output.contains("Created project"));

        // List shows both the seeded "Default" project and our new one
        let list_result = list_projects(&api_client, None, None, None, "json").await;
        assert!(list_result.is_ok());

        let output = list_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2); // Default + Test Project
    }
}
