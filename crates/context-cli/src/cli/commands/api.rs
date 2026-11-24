//! API server command - starts REST API + MCP + embedded frontend

use std::net::IpAddr;
use std::path::PathBuf;

use miette::{IntoDiagnostic, Result};

use context_core::Database;
use context_core::settings::{self, Settings};
use context_core::update::NoUpdateFlow;
use context_core::{get_config_path, get_db_path, set_base_path};
use context_db::SqliteDatabase;
use context_server::api::notifier::ChangeNotifier;
use context_server::api::{self, Config};

/// Flags the operator passed to `c5t api`.
///
/// `None` means the flag was absent, so the stored value applies. A `false`
/// `enable_docs` cannot force docs off: the stored setting still enables them.
#[derive(Debug, Clone, Default)]
pub(crate) struct ApiFlags {
    pub host: Option<IpAddr>,
    pub port: Option<u16>,
    pub skills_dir: Option<PathBuf>,
    pub verbosity: Option<u8>,
    pub enable_docs: bool,
}

/// Run the API server
pub async fn run(
    host: Option<IpAddr>,
    port: Option<u16>,
    home: Option<PathBuf>,
    skills_dir: Option<PathBuf>,
    verbosity: Option<u8>,
    enable_docs: bool,
) -> Result<()> {
    // Set the global base path if provided (API startup singleton pattern)
    if let Some(home_path) = home {
        set_base_path(home_path);
    }

    // Use the singleton to get db path
    let db_path = get_db_path();

    println!("Opening database at {:?}", db_path);

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).into_diagnostic()?;
    }

    let db = SqliteDatabase::open(&db_path).await?;

    // Run migrations before starting the server
    db.migrate()?;
    println!("Database migrations complete");

    let flags = ApiFlags {
        host,
        port,
        skills_dir,
        verbosity,
        enable_docs,
    };
    let config = effective_config(&flags, &stored_settings());

    // Print startup banner BEFORE starting server (before logging is initialized)
    println!();
    println!("🚀 c5t API server starting...");
    println!("   API:      http://{}:{}/api/v1", config.host, config.port);
    println!("   MCP:      http://{}:{}/mcp", config.host, config.port);
    println!("   Frontend: http://{}:{}/", config.host, config.port);
    if config.enable_docs {
        println!("   Docs:     http://{}:{}/docs", config.host, config.port);
    }
    println!();
    println!("   Database: {}", db_path.display());
    println!();

    // Pass the abstract Database to the API layer
    api::run(config, db, ChangeNotifier::new(), NoUpdateFlow)
        .await
        .into_diagnostic()?;

    Ok(())
}

/// Settings the runner works with: stored values over the built-in defaults.
///
/// A file that cannot be read or parsed, or that carries values outside the
/// accepted range, is reported and ignored: the server still starts, on the
/// built-in defaults.
pub(crate) fn stored_settings() -> Settings {
    let path = get_config_path();

    let stored = match settings::load(&path) {
        Ok(stored) => stored,
        Err(error) => {
            eprintln!("c5t api: ignoring settings file: {error}");
            return Settings::default();
        }
    };

    if let Err(error) = settings::validate_stored(&stored) {
        eprintln!(
            "c5t api: ignoring settings file {}: {error}",
            path.display()
        );
        return Settings::default();
    }

    settings::resolve(&stored)
}

/// Precedence: flags > `C5T_SKILLS_DIR` > stored > built-in.
///
/// The environment variable behaves like an explicit flag for `skills_dir`
/// only, so the stored value never overrides it.
pub(crate) fn effective_config(flags: &ApiFlags, stored: &Settings) -> Config {
    Config {
        host: flags.host.unwrap_or(stored.host),
        port: flags.port.unwrap_or(stored.port),
        verbosity: flags.verbosity.unwrap_or(stored.verbosity),
        enable_docs: flags.enable_docs || stored.enable_docs,
        skills_dir: flags
            .skills_dir
            .clone()
            .or_else(skills_dir_from_env)
            .unwrap_or_else(|| stored.skills_dir.clone()),
        host_fixed: false,
    }
}

/// `C5T_SKILLS_DIR` as a path, when the operator set it.
fn skills_dir_from_env() -> Option<PathBuf> {
    std::env::var("C5T_SKILLS_DIR").ok().map(PathBuf::from)
}

/// Verbosity override from the `-v` count.
///
/// clap counts zero occurrences as `0`; zero means the flag was absent, so the
/// stored verbosity applies. There is no way to force verbosity 0 on purpose.
pub(crate) fn verbosity_from_count(count: u8) -> Option<u8> {
    (count > 0).then_some(count)
}
