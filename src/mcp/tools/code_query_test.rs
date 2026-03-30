//! TDD tests for code query MCP tools
//!
//! Following TDD: RED → GREEN → REFACTOR
//! Tests written FIRST before implementation

use crate::mcp::tools::code_query::{
    CodeQueryTools, DescribeSchemaParams, ListQueriesParams, MockNanographCli,
    QueryCodeGraphParams, parse_query_metadata,
};
use mockall::predicate::*;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::RawContent;
use serde_json::json;
use std::process::{Command, Output};

// Helper to setup temp analysis directory and mock get_analysis_path
fn setup_temp_analysis(mock_cli: &mut MockNanographCli) -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().unwrap();
    let analysis_path = temp_dir.path().join("analysis.nano");
    std::fs::create_dir_all(&analysis_path).unwrap();

    // Create queries directory adjacent to analysis.nano (not inside it)
    let queries_dir = temp_dir.path().join("queries");
    std::fs::create_dir_all(&queries_dir).unwrap();

    let temp_path = temp_dir.path().to_path_buf();
    mock_cli
        .expect_get_analysis_path()
        .returning(move |_| temp_path.clone());

    temp_dir
}

// Helper to create a saved query file in the temp queries directory
fn create_saved_query(temp_dir: &tempfile::TempDir, query_name: &str, query_content: &str) {
    let queries_dir = temp_dir.path().join("queries");
    // Use sanitized query name as filename
    let sanitized = sanitize_filename::sanitize(query_name);
    let query_file = queries_dir.join(format!("{}.gq", sanitized));
    std::fs::write(query_file, query_content).unwrap();
}

// Helper to create mock successful output - cross-platform
fn mock_success_output(stdout: &str) -> Output {
    // Use actual command to get real ExitStatus
    let status = Command::new("true").status().unwrap();
    Output {
        status,
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

// Helper to create mock failed output - cross-platform
fn mock_failed_output(_code: i32, stderr: &str) -> Output {
    // Use actual command to get real ExitStatus
    let status = Command::new("false").status().unwrap();
    Output {
        status,
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

// ============================================================================
// TDD: Tool 1 - c5t_code_describe_schema
// ============================================================================

#[tokio::test]
async fn test_describe_schema_success() {
    // Arrange
    let mut mock_cli = MockNanographCli::new();
    let _temp_dir = setup_temp_analysis(&mut mock_cli);

    let schema_json = json!({
        "nodes": [
            {
                "name": "Symbol",
                "properties": [
                    {"name": "symbol_id", "type": "String"},
                    {"name": "name", "type": "String"},
                    {"name": "kind", "type": "String"},
                ]
            }
        ],
        "edges": [
            {"name": "calls", "from": "Symbol", "to": "Symbol"}
        ]
    });

    mock_cli
        .expect_describe()
        .times(1)
        .returning(move |_| Ok(mock_success_output(&schema_json.to_string())));

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = DescribeSchemaParams {
        repo_id: "7104e891".to_string(),
    };

    // Act
    let result = tools
        .describe_schema(Parameters(params))
        .await
        .expect("describe_schema should succeed");

    // Assert
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert!(response["nodes"].is_array());
    assert!(response["edges"].is_array());
}

#[tokio::test]
async fn test_describe_schema_nanograph_not_found() {
    // Arrange
    let mut mock_cli = MockNanographCli::new();
    let _temp_dir = setup_temp_analysis(&mut mock_cli);

    mock_cli.expect_describe().times(1).returning(|_| {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "nanograph: command not found",
        ))
    });

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = DescribeSchemaParams {
        repo_id: "7104e891".to_string(),
    };

    // Act
    let result = tools.describe_schema(Parameters(params)).await;

    // Assert
    assert!(result.is_err());
}

#[tokio::test]
async fn test_describe_schema_database_not_found() {
    // Arrange - This test checks that nonexistent repos return early
    let mut mock_cli = MockNanographCli::new();

    // Return a path that doesn't have analysis.nano directory
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    mock_cli
        .expect_get_analysis_path()
        .returning(move |_| temp_path.clone());

    // describe() should NOT be called because analysis.nano doesn't exist

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = DescribeSchemaParams {
        repo_id: "nonexistent".to_string(),
    };

    // Act
    let result = tools.describe_schema(Parameters(params)).await;

    // Assert
    assert!(result.is_err());
    // Implementation returns early before calling CLI, so no describe() expectation needed
}

// ============================================================================
// TDD: Tool 2 - c5t_code_query_graph
// ============================================================================

#[tokio::test]
async fn test_query_graph_with_temp_query() {
    // Arrange
    let mut mock_cli = MockNanographCli::new();

    // Create temp analysis path
    let temp_dir = tempfile::tempdir().unwrap();
    let analysis_path = temp_dir.path().join("analysis.nano");
    std::fs::create_dir_all(&analysis_path).unwrap();

    let query_result = json!([
        {
            "name": "analyze_repository",
            "kind": "function",
            "file_path": "src/analysis/service.rs",
            "start_line": 42
        }
    ]);

    let temp_path = temp_dir.path().to_path_buf();
    mock_cli
        .expect_get_analysis_path()
        .times(1)
        .returning(move |_| temp_path.clone());

    mock_cli
        .expect_run_query()
        .times(1)
        .returning(move |_, _, _, _| Ok(mock_success_output(&query_result.to_string())));

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = QueryCodeGraphParams {
        repo_id: "7104e891".to_string(),
        query_name: None,
        query_definition: Some(
            r#"query temp($n: String) {
                match { $s: Symbol, $s.name = $n }
                return { $s.name, $s.kind, $s.file_path, $s.start_line }
            }"#
            .to_string(),
        ),
        params: Some(json!({"n": "analyze_repository"})),
    };

    // Act
    let result = tools
        .query_graph(Parameters(params))
        .await
        .expect("query_graph should succeed");

    // Assert
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert!(response["results"].is_array());
    assert_eq!(response["results"][0]["name"], "analyze_repository");
    assert_eq!(response["query_type"], "temporary");
}

#[tokio::test]
async fn test_query_graph_with_saved_query_name_only() {
    // Arrange
    let mut mock_cli = MockNanographCli::new();
    let temp_dir = setup_temp_analysis(&mut mock_cli);

    // Create the saved query file
    create_saved_query(
        &temp_dir,
        "find_by_name",
        r#"
        query find_by_name($n: String) {
            match { $s: Symbol, $s.name = $n }
            return { $s.name }
        }
    "#,
    );

    let query_result = json!([
        {"name": "CodeGraph", "kind": "struct"}
    ]);

    mock_cli
        .expect_run_query()
        .times(1)
        .returning(move |_, _, query_name, _| {
            assert_eq!(query_name, "find_by_name");
            Ok(mock_success_output(&query_result.to_string()))
        });

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = QueryCodeGraphParams {
        repo_id: "7104e891".to_string(),
        query_name: Some("find_by_name".to_string()),
        query_definition: None,
        params: Some(json!({"n": "CodeGraph"})),
    };

    // Act
    let result = tools
        .query_graph(Parameters(params))
        .await
        .expect("query_graph should succeed");

    // Assert
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(response["query_type"], "saved");
    assert_eq!(response["query_name"], "find_by_name");
    assert!(response["results"].is_array());
}

#[tokio::test]
async fn test_query_graph_save_and_execute() {
    // Arrange
    let mut mock_cli = MockNanographCli::new();
    let _temp_dir = setup_temp_analysis(&mut mock_cli);

    // Expect check (validation) call
    mock_cli
        .expect_check_query()
        .times(1)
        .returning(|_, _| Ok(mock_success_output("Query is valid")));

    // Expect run call
    let query_result = json!([{"name": "test"}]);
    mock_cli
        .expect_run_query()
        .times(1)
        .returning(move |_, _, _, _| Ok(mock_success_output(&query_result.to_string())));

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = QueryCodeGraphParams {
        repo_id: "7104e891".to_string(),
        query_name: Some("new_query".to_string()),
        query_definition: Some(
            r#"query new_query($n: String)
                @description("Test query")
                @instruction("Use for testing")
            {
                match { $s: Symbol, $s.name = $n }
                return { $s.name }
            }"#
            .to_string(),
        ),
        params: Some(json!({"n": "test"})),
    };

    // Act
    let result = tools
        .query_graph(Parameters(params))
        .await
        .expect("query_graph should succeed");

    // Assert
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(response["query_type"], "saved_and_executed");
    assert_eq!(response["query_name"], "new_query");
    assert!(response["results"].is_array());
}

#[tokio::test]
async fn test_query_graph_neither_name_nor_definition() {
    // Arrange
    let mock_cli = MockNanographCli::new();
    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = QueryCodeGraphParams {
        repo_id: "7104e891".to_string(),
        query_name: None,
        query_definition: None,
        params: None,
    };

    // Act
    let result = tools.query_graph(Parameters(params)).await;

    // Assert
    assert!(result.is_err());
}

// ============================================================================
// TDD: parse_query_metadata
// ============================================================================

#[test]
fn test_parse_query_metadata_with_params() {
    let content = r#"query callers($name: String)
  @description("Find all functions that call a given function")
  @instruction("Use to trace who depends on a function")
{
  match { $caller: Symbol }
  return { $caller.name }
}"#;

    let meta = parse_query_metadata(content).expect("should parse");
    assert_eq!(meta.name, "callers");
    assert_eq!(
        meta.description.as_deref(),
        Some("Find all functions that call a given function")
    );
    assert_eq!(
        meta.instruction.as_deref(),
        Some("Use to trace who depends on a function")
    );
    assert_eq!(meta.params.len(), 1);
    assert_eq!(meta.params[0].name, "name");
    assert_eq!(meta.params[0].param_type, "String");
}

#[test]
fn test_parse_query_metadata_no_params() {
    let content = r#"query overview()
  @description("High-level codebase overview")
  @instruction("Run this first")
{
  match { $s: Symbol }
  return { $s.kind }
}"#;

    let meta = parse_query_metadata(content).expect("should parse");
    assert_eq!(meta.name, "overview");
    assert_eq!(
        meta.description.as_deref(),
        Some("High-level codebase overview")
    );
    assert_eq!(meta.instruction.as_deref(), Some("Run this first"));
    assert!(meta.params.is_empty());
}

#[test]
fn test_parse_query_metadata_multiple_params() {
    let content = r#"query find($name: String, $kind: String)
  @description("Find by name and kind")
{
  match { $s: Symbol { name: $name, kind: $kind } }
  return { $s.name }
}"#;

    let meta = parse_query_metadata(content).expect("should parse");
    assert_eq!(meta.name, "find");
    assert_eq!(meta.params.len(), 2);
    assert_eq!(meta.params[0].name, "name");
    assert_eq!(meta.params[0].param_type, "String");
    assert_eq!(meta.params[1].name, "kind");
    assert_eq!(meta.params[1].param_type, "String");
}

#[test]
fn test_parse_query_metadata_no_annotations() {
    let content = r#"query simple()
{
  match { $s: Symbol }
  return { $s.name }
}"#;

    let meta = parse_query_metadata(content).expect("should parse");
    assert_eq!(meta.name, "simple");
    assert!(meta.description.is_none());
    assert!(meta.instruction.is_none());
    assert!(meta.params.is_empty());
}

#[test]
fn test_parse_query_metadata_not_a_query() {
    assert!(parse_query_metadata("not a query").is_none());
    assert!(parse_query_metadata("").is_none());
    assert!(parse_query_metadata("match { $s: Symbol }").is_none());
}

// ============================================================================
// TDD: Tool 3 - list_queries
// ============================================================================

#[tokio::test]
async fn test_list_queries_returns_all_queries() {
    let mut mock_cli = MockNanographCli::new();
    let temp_dir = setup_temp_analysis(&mut mock_cli);

    create_saved_query(
        &temp_dir,
        "overview",
        r#"query overview()
  @description("High-level codebase overview")
  @instruction("Run this first")
{
  match { $s: Symbol }
  return { $s.kind as kind, count($s) as total }
}"#,
    );

    create_saved_query(
        &temp_dir,
        "callers",
        r#"query callers($name: String)
  @description("Find all callers of a function")
  @instruction("Use to trace dependencies")
{
  match { $c: Symbol, $t: Symbol { name: $name }, $c calls $t }
  return { $c.name }
}"#,
    );

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = ListQueriesParams {
        repo_id: "7104e891".to_string(),
    };

    let result = tools
        .list_queries(Parameters(params))
        .await
        .expect("list_queries should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(response["total"], 2);
    assert_eq!(response["repo_id"], "7104e891");

    let queries = response["queries"].as_array().unwrap();
    assert_eq!(queries.len(), 2);

    // Sorted alphabetically
    assert_eq!(queries[0]["name"], "callers");
    assert_eq!(queries[0]["description"], "Find all callers of a function");
    assert_eq!(queries[0]["params"][0]["name"], "name");
    assert_eq!(queries[0]["params"][0]["param_type"], "String");

    assert_eq!(queries[1]["name"], "overview");
    assert_eq!(queries[1]["description"], "High-level codebase overview");
    assert!(queries[1]["params"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_queries_no_queries_dir() {
    let mut mock_cli = MockNanographCli::new();

    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    mock_cli
        .expect_get_analysis_path()
        .returning(move |_| temp_path.clone());

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = ListQueriesParams {
        repo_id: "nonexistent".to_string(),
    };

    let result = tools.list_queries(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_queries_skips_non_gq_files() {
    let mut mock_cli = MockNanographCli::new();
    let temp_dir = setup_temp_analysis(&mut mock_cli);

    create_saved_query(
        &temp_dir,
        "overview",
        r#"query overview()
  @description("Overview")
{
  match { $s: Symbol }
  return { $s.kind }
}"#,
    );

    // Create non-.gq files that should be ignored
    let queries_dir = temp_dir.path().join("queries");
    std::fs::write(queries_dir.join("README.md"), "# Queries").unwrap();
    std::fs::write(queries_dir.join("notes.txt"), "some notes").unwrap();

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = ListQueriesParams {
        repo_id: "7104e891".to_string(),
    };

    let result = tools
        .list_queries(Parameters(params))
        .await
        .expect("list_queries should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test]
async fn test_list_queries_empty_dir() {
    let mut mock_cli = MockNanographCli::new();
    let _temp_dir = setup_temp_analysis(&mut mock_cli);

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = ListQueriesParams {
        repo_id: "7104e891".to_string(),
    };

    let result = tools
        .list_queries(Parameters(params))
        .await
        .expect("list_queries should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 0);
    assert!(response["queries"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_query_graph_invalid_query_syntax() {
    // Arrange
    let mut mock_cli = MockNanographCli::new();
    let _temp_dir = setup_temp_analysis(&mut mock_cli);

    mock_cli.expect_check_query().times(1).returning(|_, _| {
        Ok(mock_failed_output(
            1,
            "Syntax error at line 2: unexpected token",
        ))
    });

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = QueryCodeGraphParams {
        repo_id: "7104e891".to_string(),
        query_name: Some("bad_query".to_string()),
        query_definition: Some("query bad { invalid syntax }".to_string()),
        params: None,
    };

    // Act
    let result = tools.query_graph(Parameters(params)).await;

    // Assert
    assert!(result.is_err());
}

#[tokio::test]
async fn test_query_graph_with_empty_params() {
    // Arrange
    let mut mock_cli = MockNanographCli::new();
    let temp_dir = setup_temp_analysis(&mut mock_cli);

    // Create the saved query file
    create_saved_query(
        &temp_dir,
        "count_all",
        r#"
        query count_all {
            match { $s: Symbol }
            return { name: "all_symbols", count: count($s) }
        }
    "#,
    );

    let query_result = json!([
        {"name": "all_symbols", "count": 1656}
    ]);

    mock_cli
        .expect_run_query()
        .times(1)
        .withf(|_, _, _, params| params.is_empty())
        .returning(move |_, _, _, _| Ok(mock_success_output(&query_result.to_string())));

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = QueryCodeGraphParams {
        repo_id: "7104e891".to_string(),
        query_name: Some("count_all".to_string()),
        query_definition: None,
        params: None,
    };

    // Act
    let result = tools
        .query_graph(Parameters(params))
        .await
        .expect("query_graph should succeed");

    // Assert - should work with no params
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert!(response["results"].is_array());
}

#[tokio::test]
async fn test_query_graph_malformed_json_result() {
    // Arrange
    let mut mock_cli = MockNanographCli::new();
    let temp_dir = setup_temp_analysis(&mut mock_cli);

    // Create the saved query file
    create_saved_query(
        &temp_dir,
        "test",
        r#"
        query test {
            match { $s: Symbol }
            return { $s.name }
        }
    "#,
    );

    mock_cli
        .expect_run_query()
        .times(1)
        .returning(|_, _, _, _| Ok(mock_success_output("{ invalid json }")));

    let tools = CodeQueryTools::new_with_cli(mock_cli);

    let params = QueryCodeGraphParams {
        repo_id: "7104e891".to_string(),
        query_name: Some("test".to_string()),
        query_definition: None,
        params: None,
    };

    // Act
    let result = tools.query_graph(Parameters(params)).await;

    // Assert
    assert!(result.is_err());
}
