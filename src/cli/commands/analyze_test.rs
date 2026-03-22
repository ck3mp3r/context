use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::analyze::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use tempfile::TempDir;
use tokio::net::TcpListener;

/// Spawn a test HTTP server with in-memory database
async fn spawn_test_server() -> (String, tokio::task::JoinHandle<()>) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");
    let temp_dir = TempDir::new().unwrap();
    // Create job infrastructure
    let job_queue = crate::jobs::JobQueue::new();
    let job_registry = crate::jobs::JobRegistry::new();
    let job_executor = crate::jobs::JobExecutor::new(job_queue.clone(), job_registry);
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
        job_queue,
        job_executor,
    );
    let app = routes::create_router(state, false);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (url, handle)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_analyze_repo_not_found() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    let args = AnalyzeArgs {
        repo: "nonexistent-repo".to_string(),
        poll_interval: 1,
    };

    let result = analyze(&api_client, args).await;
    assert!(result.is_err(), "Should fail for nonexistent repo");
    assert!(
        result.unwrap_err().to_string().contains("not found"),
        "Error should mention repo not found"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_analyze_args_structure() {
    // Verify that AnalyzeArgs has the expected structure
    let args = AnalyzeArgs {
        repo: "test-repo".to_string(),
        poll_interval: 5,
    };

    assert_eq!(args.repo, "test-repo");
    assert_eq!(args.poll_interval, 5);
}
