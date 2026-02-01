use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::error::{CliError, CliResult};
use crate::cli::utils::{apply_table_style, format_tags, truncate_with_ellipsis};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};

#[derive(Debug, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub tags: Vec<String>,
    pub project_ids: Vec<String>,
    pub scripts: Vec<String>,
    pub references: Vec<String>,
    pub assets: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct UpdateSkillRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // total, limit, offset are part of API contract but not used in CLI
struct SkillListResponse {
    items: Vec<Skill>,
    total: usize,
    limit: usize,
    offset: usize,
}

#[derive(Tabled)]
pub(crate) struct SkillDisplay {
    #[tabled(rename = "ID")]
    pub(crate) id: String,
    #[tabled(rename = "Name")]
    pub(crate) name: String,
    #[tabled(rename = "Tags")]
    pub(crate) tags: String,
}

impl From<&Skill> for SkillDisplay {
    fn from(skill: &Skill) -> Self {
        Self {
            id: skill.id.clone(),
            name: truncate_with_ellipsis(&skill.name, 50),
            tags: format_tags(Some(&skill.tags)),
        }
    }
}

/// List skills with optional filtering
#[allow(clippy::too_many_arguments)]
pub async fn list_skills(
    api_client: &ApiClient,
    project_id: Option<&str>,
    tags: Option<&str>,
    page: PageParams<'_>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get("/api/v1/skills");

    if let Some(pid) = project_id {
        request = request.query(&[("project_id", pid)]);
    }
    if let Some(t) = tags {
        request = request.query(&[("tags", t)]);
    }
    if let Some(l) = page.limit {
        request = request.query(&[("limit", l.to_string().as_str())]);
    }
    if let Some(o) = page.offset {
        request = request.query(&[("offset", o.to_string().as_str())]);
    }
    if let Some(s) = page.sort {
        request = request.query(&[("sort", s)]);
    }
    if let Some(ord) = page.order {
        request = request.query(&[("order", ord)]);
    }

    let response: SkillListResponse = request
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| CliError::InvalidResponse {
            message: format!("Failed to parse response: {}", e),
        })?;

    if format == "json" {
        Ok(serde_json::to_string_pretty(&response.items)?)
    } else {
        let display: Vec<SkillDisplay> = response.items.iter().map(SkillDisplay::from).collect();
        let mut table = Table::new(display);
        apply_table_style(&mut table);
        Ok(format!("{}", table))
    }
}

/// Get a skill by ID
pub async fn get_skill(api_client: &ApiClient, id: &str, format: &str) -> CliResult<String> {
    let skill: Skill = api_client
        .get(&format!("/api/v1/skills/{}", id))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| CliError::InvalidResponse {
            message: format!("Failed to parse response: {}", e),
        })?;

    if format == "json" {
        Ok(serde_json::to_string_pretty(&skill)?)
    } else {
        let output = format!(
            "ID: {}\nName: {}\nDescription: {}\nTags: {}\nProject IDs: {}\nCreated: {}\nUpdated: {}\n\nContent:\n{}",
            skill.id,
            skill.name,
            &skill.description,
            format_tags(Some(&skill.tags)),
            if skill.project_ids.is_empty() {
                "N/A".to_string()
            } else {
                skill.project_ids.join(", ")
            },
            skill.created_at,
            skill.updated_at,
            skill.content
        );

        Ok(output)
    }
}

/// Create a new skill
pub async fn create_skill(
    api_client: &ApiClient,
    request: CreateSkillRequest,
) -> CliResult<String> {
    let skill: Skill = api_client
        .post("/api/v1/skills")
        .json(&request)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| CliError::InvalidResponse {
            message: format!("Failed to parse response: {}", e),
        })?;

    Ok(format!("Created skill: {} (ID: {})", skill.name, skill.id))
}

/// Update a skill
pub async fn update_skill(
    api_client: &ApiClient,
    id: &str,
    request: UpdateSkillRequest,
) -> CliResult<String> {
    let skill: Skill = api_client
        .patch(&format!("/api/v1/skills/{}", id))
        .json(&request)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| CliError::InvalidResponse {
            message: format!("Failed to parse response: {}", e),
        })?;

    Ok(format!("Updated skill: {} (ID: {})", skill.name, skill.id))
}

/// Delete a skill
pub async fn delete_skill(api_client: &ApiClient, id: &str, force: bool) -> CliResult<String> {
    // Safety check: require --force flag
    if !force {
        return Err(CliError::InvalidResponse {
            message: "Delete operation requires --force flag. This action is destructive and cannot be undone.".to_string(),
        });
    }

    api_client
        .delete(&format!("/api/v1/skills/{}", id))
        .send()
        .await?
        .error_for_status()?;

    Ok(format!("Deleted skill: {}", id))
}

/// Import a skill from a source
///
/// Source formats:
/// - Local: ./path/to/skill, /abs/path/to/skill, file:///path/to/skill
/// - Git: git+https://github.com/user/repo (FUTURE - not yet implemented)
///
/// The path within the source can be specified via --path flag
pub async fn import_skill(
    api_client: &ApiClient,
    source: &str,
    path_override: Option<&str>,
    project_ids: Option<Vec<String>>,
) -> CliResult<String> {
    #[derive(Serialize)]
    struct ImportRequest {
        source: String,
        path: Option<String>,
        project_ids: Option<Vec<String>>,
    }

    let request = ImportRequest {
        source: source.to_string(),
        path: path_override.map(|s| s.to_string()),
        project_ids,
    };

    let response = api_client
        .post("/api/v1/skills/import")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(CliError::ApiError {
            status: status.as_u16(),
            message: format!("Import failed: {}", error_text),
        });
    }

    let skill: Skill = response
        .json()
        .await
        .map_err(|e| CliError::InvalidResponse {
            message: format!("Failed to parse response: {}", e),
        })?;

    Ok(format!("Imported skill: {} (ID: {})", skill.name, skill.id))
}
