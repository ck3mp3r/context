// Test fixture for import extraction

// Simple use statement
use std::collections::HashMap;

// Multiple imports from same module
use std::fs::File;
use std::io::Read;

// Nested imports
use std::{env, path::PathBuf};

// Crate-relative import
use crate::db::Database;

// Super-relative import
use super::utils::helper;

// Self import
use self::inner::function;

// Aliased import
use std::io::Result as IoResult;

// Pub use (re-export)
pub use crate::types::Symbol;

// Glob import
use std::prelude::*;

// Deep nested imports
use std::collections::{BTreeMap, HashMap, HashSet};

// Function using imports
fn main() {
    let map = HashMap::new();
    let file = File::open("test.txt");
}
