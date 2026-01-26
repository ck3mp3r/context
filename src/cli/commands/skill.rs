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
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub tags: Vec<String>,
    pub project_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSkillRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
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
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
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

    let skills: Vec<Skill> = request
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| CliError::InvalidResponse {
            message: format!("Failed to parse response: {}", e),
        })?;

    if format == "json" {
        Ok(serde_json::to_string_pretty(&skills)?)
    } else {
        let display: Vec<SkillDisplay> = skills.iter().map(SkillDisplay::from).collect();
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
        Ok(format!(
            "ID: {}\nName: {}\nDescription: {}\nInstructions: {}\nTags: {}\nProject IDs: {}\nCreated: {}\nUpdated: {}",
            skill.id,
            skill.name,
            skill.description.as_deref().unwrap_or("N/A"),
            skill.instructions.as_deref().unwrap_or("N/A"),
            format_tags(Some(&skill.tags)),
            if skill.project_ids.is_empty() {
                "N/A".to_string()
            } else {
                skill.project_ids.join(", ")
            },
            skill.created_at,
            skill.updated_at
        ))
    }
}

/// Create a new skill
pub async fn create_skill(
    api_client: &ApiClient,
    name: &str,
    description: Option<&str>,
    instructions: Option<&str>,
    tags: Option<&str>,
    project_ids: Option<&str>,
) -> CliResult<String> {
    let req = CreateSkillRequest {
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        instructions: instructions.map(|s| s.to_string()),
        tags: tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect()),
        project_ids: project_ids.map(|p| p.split(',').map(|s| s.trim().to_string()).collect()),
    };

    let skill: Skill = api_client
        .post("/api/v1/skills")
        .json(&req)
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
    name: Option<&str>,
    description: Option<&str>,
    instructions: Option<&str>,
    tags: Option<&str>,
    project_ids: Option<&str>,
) -> CliResult<String> {
    let req = UpdateSkillRequest {
        name: name.map(|s| s.to_string()),
        description: description.map(|s| s.to_string()),
        instructions: instructions.map(|s| s.to_string()),
        tags: tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect()),
        project_ids: project_ids.map(|p| p.split(',').map(|s| s.trim().to_string()).collect()),
    };

    let skill: Skill = api_client
        .patch(&format!("/api/v1/skills/{}", id))
        .json(&req)
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
