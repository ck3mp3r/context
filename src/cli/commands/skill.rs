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

/// Filter parameters for listing skills
pub struct ListSkillsFilter<'a> {
    pub query: Option<&'a str>,
    pub project_id: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub page: PageParams<'a>,
}

/// List skills with optional filtering
pub async fn list_skills(
    api_client: &ApiClient,
    filter: ListSkillsFilter<'_>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get("/api/v1/skills");

    if let Some(q) = filter.query {
        request = request.query(&[("q", q)]);
    }
    if let Some(pid) = filter.project_id {
        request = request.query(&[("project_id", pid)]);
    }
    if let Some(t) = filter.tags {
        request = request.query(&[("tags", t)]);
    }
    if let Some(l) = filter.page.limit {
        request = request.query(&[("limit", l.to_string().as_str())]);
    }
    if let Some(o) = filter.page.offset {
        request = request.query(&[("offset", o.to_string().as_str())]);
    }
    if let Some(s) = filter.page.sort {
        request = request.query(&[("sort", s)]);
    }
    if let Some(ord) = filter.page.order {
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
    tags: Option<Vec<String>>,
    update: bool,
) -> CliResult<String> {
    #[derive(Serialize)]
    struct ImportRequest {
        source: String,
        path: Option<String>,
        project_ids: Option<Vec<String>>,
        tags: Option<Vec<String>>,
        update: bool,
    }

    let request = ImportRequest {
        source: source.to_string(),
        path: path_override.map(|s| s.to_string()),
        project_ids,
        tags,
        update,
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

/// Update skill metadata (tags, project_ids)
pub async fn update_skill(
    api_client: &ApiClient,
    skill_id: &str,
    tags: Option<Vec<String>>,
    project_ids: Option<Vec<String>>,
) -> CliResult<String> {
    #[derive(Serialize)]
    struct UpdateRequest {
        tags: Option<Vec<String>>,
        project_ids: Option<Vec<String>>,
    }

    let request = UpdateRequest { tags, project_ids };

    let response = api_client
        .patch(&format!("/api/v1/skills/{}", skill_id))
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
            message: format!("Update failed: {}", error_text),
        });
    }

    let skill: Skill = response
        .json()
        .await
        .map_err(|e| CliError::InvalidResponse {
            message: format!("Failed to parse response: {}", e),
        })?;

    Ok(format!("Updated skill: {} (ID: {})", skill.name, skill.id))
}
