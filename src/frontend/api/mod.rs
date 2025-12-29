// Allow dead code in API module - this is a library of functions used incrementally
#![allow(dead_code)]

use gloo_net::http::Request;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::models::{ApiError, Note, Paginated, Project, Repo, Task, TaskList, TaskStats};

const API_BASE: &str = "/api/v1";

/// API client error type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiClientError {
    Network(String),
    Server(ApiError),
    Deserialization(String),
}

impl std::fmt::Display for ApiClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiClientError::Network(msg) => write!(f, "Network error: {}", msg),
            ApiClientError::Server(err) => write!(f, "Server error: {}", err.error),
            ApiClientError::Deserialization(msg) => write!(f, "Deserialization error: {}", msg),
        }
    }
}

type Result<T> = std::result::Result<T, ApiClientError>;

/// Helper function to handle API responses
async fn handle_response<T: DeserializeOwned>(
    request: gloo_net::http::RequestBuilder,
) -> Result<T> {
    let response = request
        .send()
        .await
        .map_err(|e| ApiClientError::Network(e.to_string()))?;

    let status = response.status();

    if (200..300).contains(&status) {
        response
            .json::<T>()
            .await
            .map_err(|e| ApiClientError::Deserialization(e.to_string()))
    } else {
        let error = response
            .json::<ApiError>()
            .await
            .map_err(|e| ApiClientError::Deserialization(e.to_string()))?;
        Err(ApiClientError::Server(error))
    }
}

/// Projects API
pub mod projects {
    use super::*;

    pub async fn list(limit: Option<usize>, offset: Option<usize>) -> Result<Paginated<Project>> {
        let mut url = format!("{}/projects", API_BASE);
        let mut query_params = vec![];

        if let Some(lim) = limit {
            query_params.push(format!("limit={}", lim));
        }
        if let Some(off) = offset {
            query_params.push(format!("offset={}", off));
        }

        if !query_params.is_empty() {
            url = format!("{}?{}", url, query_params.join("&"));
        }

        handle_response(Request::get(&url)).await
    }

    pub async fn get(id: &str) -> Result<Project> {
        let url = format!("{}/projects/{}", API_BASE, id);
        handle_response(Request::get(&url)).await
    }

    pub async fn delete(id: &str) -> Result<()> {
        let url = format!("{}/projects/{}", API_BASE, id);
        let response = Request::delete(&url)
            .send()
            .await
            .map_err(|e| ApiClientError::Network(e.to_string()))?;

        if response.status() >= 200 && response.status() < 300 {
            Ok(())
        } else {
            let error = response
                .json::<ApiError>()
                .await
                .map_err(|e| ApiClientError::Deserialization(e.to_string()))?;
            Err(ApiClientError::Server(error))
        }
    }
}

/// Repos API
pub mod repos {
    use super::*;

    pub async fn list(
        limit: Option<usize>,
        offset: Option<usize>,
        project_id: Option<String>,
    ) -> Result<Paginated<Repo>> {
        let mut url = format!("{}/repos", API_BASE);
        let mut query_params = vec![];

        if let Some(proj_id) = project_id {
            query_params.push(format!("project_id={}", proj_id));
        }
        if let Some(lim) = limit {
            query_params.push(format!("limit={}", lim));
        }
        if let Some(off) = offset {
            query_params.push(format!("offset={}", off));
        }

        if !query_params.is_empty() {
            url = format!("{}?{}", url, query_params.join("&"));
        }

        handle_response(Request::get(&url)).await
    }

    pub async fn get(id: &str) -> Result<Repo> {
        let url = format!("{}/repos/{}", API_BASE, id);
        handle_response(Request::get(&url)).await
    }

    pub async fn delete(id: &str) -> Result<()> {
        let url = format!("{}/repos/{}", API_BASE, id);
        let response = Request::delete(&url)
            .send()
            .await
            .map_err(|e| ApiClientError::Network(e.to_string()))?;

        if response.status() >= 200 && response.status() < 300 {
            Ok(())
        } else {
            let error = response
                .json::<ApiError>()
                .await
                .map_err(|e| ApiClientError::Deserialization(e.to_string()))?;
            Err(ApiClientError::Server(error))
        }
    }
}

/// Task Lists API
pub mod task_lists {
    use super::*;

    pub async fn list(
        limit: Option<usize>,
        offset: Option<usize>,
        project_id: Option<String>,
        status: Option<&str>,
    ) -> Result<Paginated<TaskList>> {
        let mut url = format!("{}/task-lists", API_BASE);
        let mut query_params = vec![];

        if let Some(proj_id) = project_id {
            query_params.push(format!("project_id={}", proj_id));
        }
        if let Some(stat) = status {
            query_params.push(format!("status={}", stat));
        }
        if let Some(lim) = limit {
            query_params.push(format!("limit={}", lim));
        }
        if let Some(off) = offset {
            query_params.push(format!("offset={}", off));
        }

        if !query_params.is_empty() {
            url = format!("{}?{}", url, query_params.join("&"));
        }

        handle_response(Request::get(&url)).await
    }

    pub async fn get(id: &str) -> Result<TaskList> {
        let url = format!("{}/task-lists/{}", API_BASE, id);
        handle_response(Request::get(&url)).await
    }

    pub async fn get_stats(id: &str) -> Result<TaskStats> {
        let url = format!("{}/task-lists/{}/stats", API_BASE, id);
        handle_response(Request::get(&url)).await
    }
}

/// Tasks API
pub mod tasks {
    use super::*;

    pub async fn list_for_task_list(
        list_id: &str,
        limit: Option<usize>,
        offset: Option<usize>,
        status: Option<&str>,
        sort: Option<&str>,
        order: Option<&str>,
        parent_id: Option<&str>,
    ) -> Result<Paginated<Task>> {
        let mut url = format!("{}/task-lists/{}/tasks", API_BASE, list_id);
        let mut query_params = vec![];

        if let Some(lim) = limit {
            query_params.push(format!("limit={}", lim));
        }
        if let Some(off) = offset {
            query_params.push(format!("offset={}", off));
        }
        if let Some(s) = status {
            query_params.push(format!("status={}", s));
        }
        if let Some(s) = sort {
            query_params.push(format!("sort={}", s));
        }
        if let Some(o) = order {
            query_params.push(format!("order={}", o));
        }
        if let Some(p) = parent_id {
            query_params.push(format!("parent_id={}", p));
        }

        if !query_params.is_empty() {
            url = format!("{}?{}", url, query_params.join("&"));
        }

        handle_response(Request::get(&url)).await
    }

    pub async fn get(id: &str) -> Result<Task> {
        let url = format!("{}/tasks/{}", API_BASE, id);
        handle_response(Request::get(&url)).await
    }
}

/// Notes API
pub mod notes {
    use super::*;

    pub async fn list(
        limit: Option<usize>,
        offset: Option<usize>,
        search_query: Option<String>,
        project_id: Option<String>,
    ) -> Result<Paginated<Note>> {
        let mut url = format!("{}/notes", API_BASE);
        let mut query_params = vec![];

        if let Some(proj_id) = project_id {
            query_params.push(format!("project_id={}", proj_id));
        }
        if let Some(q) = search_query
            && !q.trim().is_empty()
        {
            // Simple URL encoding for search query
            let encoded = q
                .replace(' ', "+")
                .replace('&', "%26")
                .replace('=', "%3D")
                .replace('#', "%23");
            query_params.push(format!("q={}", encoded));
        }
        if let Some(lim) = limit {
            query_params.push(format!("limit={}", lim));
        }
        if let Some(off) = offset {
            query_params.push(format!("offset={}", off));
        }

        if !query_params.is_empty() {
            url = format!("{}?{}", url, query_params.join("&"));
        }

        handle_response(Request::get(&url)).await
    }

    pub async fn get(id: &str) -> Result<Note> {
        let url = format!("{}/notes/{}", API_BASE, id);
        handle_response(Request::get(&url)).await
    }

    pub async fn delete(id: &str) -> Result<()> {
        let url = format!("{}/notes/{}", API_BASE, id);
        let response = Request::delete(&url)
            .send()
            .await
            .map_err(|e| ApiClientError::Network(e.to_string()))?;

        if response.status() >= 200 && response.status() < 300 {
            Ok(())
        } else {
            let error = response
                .json::<ApiError>()
                .await
                .map_err(|e| ApiClientError::Deserialization(e.to_string()))?;
            Err(ApiClientError::Server(error))
        }
    }
}
