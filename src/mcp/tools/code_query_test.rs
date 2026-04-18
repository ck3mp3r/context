//! Tests for code query MCP tools
//!
//! NOTE: These tests require a real SurrealDB database and are currently
//! integration tests. They test the 3 query modes: temporary, saved, and save-and-execute.
//!
//! To run integration tests:
//! 1. Run c5t_code_analyze on a repository first
//! 2. Use that repo_id in the tests
//!
//! For now, these tests are disabled in CI. The implementation has been verified
//! to compile and the API is correct.

#[cfg(test)]
mod placeholder {
    //! Placeholder tests to ensure the module compiles

    use crate::a6s::store::surrealdb;
    use crate::mcp::tools::code_query::{
        CodeQueryTools, DescribeSchemaParams, ListQueriesParams, QueryCodeGraphParams,
    };
    use rmcp::schemars;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tools_can_be_created() {
        let analysis_db = Arc::new(surrealdb::init_db(None).await.unwrap());
        let _tools = CodeQueryTools::new(analysis_db);
        // If this compiles, the API is correct
    }

    #[test]
    fn test_params_structures_exist() {
        let _describe_params = DescribeSchemaParams {
            repo_id: "test".to_string(),
        };

        let _list_params = ListQueriesParams {
            repo_id: "test".to_string(),
        };

        let _query_params = QueryCodeGraphParams {
            repo_id: "test".to_string(),
            query_name: None,
            query_definition: Some("SELECT * FROM file".to_string()),
            variables: None,
        };

        // Test deserialization from JSON object (simulates rmcp's from_context_part)
        let json_with_variables = serde_json::json!({
            "repo_id": "test1234",
            "query_name": "symbol_search",
            "variables": {"name": "MyClass"}
        });
        let deserialized: QueryCodeGraphParams =
            serde_json::from_value(json_with_variables).expect("should deserialize with variables");
        assert_eq!(deserialized.repo_id, "test1234");
        assert_eq!(deserialized.query_name, Some("symbol_search".to_string()));
        assert!(deserialized.variables.is_some());
        // Verify it's a proper HashMap now
        let vars = deserialized.variables.unwrap();
        assert_eq!(vars.get("name").unwrap(), &serde_json::Value::String("MyClass".to_string()));

        // All 3 modes are represented
        let _temp_query = QueryCodeGraphParams {
            query_name: None,
            query_definition: Some("SELECT *".to_string()),
            ..Default::default()
        };

        let _saved_query = QueryCodeGraphParams {
            query_name: Some("my_query".to_string()),
            query_definition: None,
            ..Default::default()
        };

        let _save_and_execute = QueryCodeGraphParams {
            query_name: Some("my_query".to_string()),
            query_definition: Some("SELECT *".to_string()),
            ..Default::default()
        };
    }

    #[test]
    fn test_schema_for_query_code_graph_params() {
        let settings = schemars::generate::SchemaSettings::draft2020_12();
        let generator = settings.into_generator();
        let schema = generator.into_root_schema_for::<QueryCodeGraphParams>();
        let schema_json = serde_json::to_value(&schema).unwrap();
        eprintln!(
            "QueryCodeGraphParams schema:\n{}",
            serde_json::to_string_pretty(&schema_json).unwrap()
        );
        assert_eq!(schema_json["type"], "object");
        assert!(schema_json["properties"]["variables"].is_object());
    }

    #[test]
    fn test_schema_for_describe_schema_params() {
        let settings = schemars::generate::SchemaSettings::draft2020_12();
        let generator = settings.into_generator();
        let schema = generator.into_root_schema_for::<DescribeSchemaParams>();
        let schema_json = serde_json::to_value(&schema).unwrap();
        eprintln!(
            "DescribeSchemaParams schema:\n{}",
            serde_json::to_string_pretty(&schema_json).unwrap()
        );
        assert_eq!(schema_json["type"], "object");
    }

    #[test]
    fn test_schema_for_hashmap_params() {
        use rmcp::schemars::JsonSchema;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, JsonSchema, Default)]
        struct TestParamsWithHashMap {
            #[serde(default)]
            pub repo_id: String,
            pub query_name: Option<String>,
            pub query_definition: Option<String>,
            pub variables: Option<std::collections::HashMap<String, String>>,
        }

        let settings = schemars::generate::SchemaSettings::draft2020_12();
        let generator = settings.into_generator();
        let schema = generator.into_root_schema_for::<TestParamsWithHashMap>();
        let schema_json = serde_json::to_value(&schema).unwrap();
        eprintln!(
            "TestParamsWithHashMap schema:\n{}",
            serde_json::to_string_pretty(&schema_json).unwrap()
        );
        assert_eq!(schema_json["type"], "object");
        assert!(schema_json["properties"]["variables"].is_object());
    }
}

// Integration tests (require real database)
// Uncomment and run manually when needed
/*
use crate::a6s::store::CodeGraph;
use crate::mcp::tools::code_query::{
    CodeQueryTools, DescribeSchemaParams, ListQueriesParams, QueryCodeGraphParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::RawContent;
use serde_json::json;

fn get_text_content(response: &rmcp::model::CallToolResult) -> &str {
    match &response.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore] // Run with: cargo test -- --ignored
async fn integration_test_temporary_query() {
    // Replace with actual analyzed repo_id
    let repo_id = "YOUR_ANALYZED_REPO_ID";

    let tools = CodeQueryTools::new();
    let params = QueryCodeGraphParams {
        repo_id: repo_id.to_string(),
        query_name: None,
        query_definition: Some("SELECT * FROM file WHERE repo_id = $repo_id".to_string()),
        params: None,
    };

    let result = tools.query_graph(Parameters(params)).await;
    assert!(result.is_ok(), "Temporary query should succeed: {:?}", result.err());

    let response = result.unwrap();
    let content = get_text_content(&response);
    let json: serde_json::Value = serde_json::from_str(content).unwrap();

    assert_eq!(json["query_type"], "temporary");
    assert!(json["results"].is_array());
}
*/
