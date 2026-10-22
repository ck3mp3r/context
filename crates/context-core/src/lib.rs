pub mod error;
pub mod jsonl;
pub mod models;
pub mod paths;
pub mod repository;
pub mod sync_types;
pub mod utils;

#[cfg(test)]
#[path = "../test/jsonl.rs"]
mod jsonl_test;
#[cfg(test)]
#[path = "../test/models.rs"]
mod models_test;
#[cfg(test)]
#[path = "../test/paths.rs"]
mod paths_test;
#[cfg(test)]
#[path = "../test/utils.rs"]
mod utils_test;

pub use error::{DbError, DbResult};
pub use jsonl::{JsonlError, JsonlReader, JsonlWriter, count_jsonl_lines, read_jsonl, write_jsonl};
pub use models::*;
pub use paths::*;
pub use repository::*;
pub use sync_types::*;
pub use utils::*;
