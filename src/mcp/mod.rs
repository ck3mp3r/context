//! Model Context Protocol (MCP) server implementation
//!
//! This module provides an MCP server using the Streamable HTTP transport.
//! The server exposes tools for managing projects, repos, task lists, tasks, and notes.
//!
//! # Architecture (SOLID Principles)
//!
//! - **server**: Main MCP server coordinator
//! - **tools**: Separate tool structs per entity (SRP - Single Responsibility)
//!   - ProjectTools: Manages project operations
//!   - RepoTools: Manages repository operations
//!   - TaskListTools: Manages task list operations
//!   - TaskTools: Manages task operations
//!   - NoteTools: Manages note operations
//!
//! Each tool struct is generic over `D: Database` (DIP - Dependency Inversion),
//! using zero-cost abstractions (no dynamic dispatch).

pub mod server;
mod service;
pub mod tools;

#[cfg(test)]
mod server_test;
#[cfg(test)]
mod service_test;

pub use server::McpServer;
pub use service::create_mcp_service;
