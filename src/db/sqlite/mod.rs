//! SQLite implementation of the database traits.
//!
//! This module provides a SQLite-backed implementation of the repository
//! traits defined in the parent module.

mod connection;

#[cfg(test)]
mod connection_test;

pub use connection::SqliteDatabase;
