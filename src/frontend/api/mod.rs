// Allow dead code in API module - this is a library of functions used incrementally
#![allow(dead_code)]

use gloo_net::http::Request;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::marker::PhantomData;

use crate::models::{ApiError, Note, Paginated, Project, Repo, Skill, Task, TaskList, TaskStats};

// Development: Trunk proxy strips /dev prefix, forwards /api/v1/* to backend
#[cfg(debug_assertions)]
const API_BASE: &str = "/dev/api/v1";

// Production: Direct calls to backend at /api/v1/*
#[cfg(not(debug_assertions))]
const API_BASE: &str = "/api/v1";

/// Trait for types that have a list endpoint
pub trait ListEndpoint: DeserializeOwned {
    fn endpoint() -> &'static str;
}

impl ListEndpoint for Project {
    fn endpoint() -> &'static str {
        "projects"
    }
}

impl ListEndpoint for Repo {
    fn endpoint() -> &'static str {
        "repos"
    }
}

impl ListEndpoint for Note {
    fn endpoint() -> &'static str {
        "notes"
    }
}

impl ListEndpoint for Skill {
    fn endpoint() -> &'static str {
        "skills"
    }
}

impl ListEndpoint for TaskList {
    fn endpoint() -> &'static str {
        "task-lists"
    }
}

/// Generic query builder for list endpoints
pub struct QueryBuilder<T: ListEndpoint> {
    limit: Option<usize>,
    offset: Option<usize>,
    search: Option<String>,
    sort: Option<String>,
    order: Option<String>,
    params: Vec<(String, String)>,
    _phantom: PhantomData<T>,
}

impl<T: ListEndpoint> QueryBuilder<T> {
    pub fn new() -> Self {
        Self {
            limit: None,
            offset: None,
            search: None,
            sort: None,
            order: None,
            params: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn search(mut self, query: impl Into<String>) -> Self {
        self.search = Some(query.into());
        self
    }

    pub fn sort(mut self, field: impl Into<String>) -> Self {
        self.sort = Some(field.into());
        self
    }

    pub fn order(mut self, order: impl Into<String>) -> Self {
        self.order = Some(order.into());
        self
    }

    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.push((key.into(), value.into()));
        self
    }

    pub async fn fetch(self) -> Result<Paginated<T>> {
        let mut url = format!("{}/{}", API_BASE, T::endpoint());
        let mut query_params = vec![];

        // Add custom params first
        for (key, value) in self.params {
            query_params.push(format!("{}={}", key, value));
        }

        // Add search query with encoding
        if let Some(q) = self.search
            && !q.trim().is_empty()
        {
            let encoded = q
                .replace(' ', "+")
                .replace('&', "%26")
                .replace('=', "%3D")
                .replace('#', "%23");
            query_params.push(format!("q={}", encoded));
        }

        // Add pagination
        if let Some(lim) = self.limit {
            query_params.push(format!("limit={}", lim));
        }
        if let Some(off) = self.offset {
            query_params.push(format!("offset={}", off));
        }

        // Add sorting
        if let Some(s) = self.sort {
            query_params.push(format!("sort={}", s));
        }
        if let Some(o) = self.order {
            query_params.push(format!("order={}", o));
        }

        if !query_params.is_empty() {
            url = format!("{}?{}", url, query_params.join("&"));
        }

        handle_response(Request::get(&url)).await
    }
}

impl<T: ListEndpoint> Default for QueryBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

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

    #[allow(clippy::too_many_arguments)]
    pub async fn list_for_task_list(
        list_id: &str,
        limit: Option<usize>,
        offset: Option<usize>,
        status: Option<&str>,
        sort: Option<&str>,
        order: Option<&str>,
        parent_id: Option<&str>,
        task_type: Option<&str>,
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
        if let Some(t) = task_type {
            query_params.push(format!("type={}", t));
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

/// Skills API
pub mod skills {
    use super::*;

    pub async fn get(id: &str) -> Result<Skill> {
        let url = format!("{}/skills/{}", API_BASE, id);
        handle_response(Request::get(&url)).await
    }

    pub async fn delete(id: &str) -> Result<()> {
        let url = format!("{}/skills/{}", API_BASE, id);
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
