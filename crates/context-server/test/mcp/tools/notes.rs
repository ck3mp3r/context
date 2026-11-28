//! Tests for the note MCP tools: range etags and per-patch etag validation.

use std::sync::Arc;

use context_core::{Database, HasNotes, Note, NoteRepository, NoteStatus};
use context_db::SqliteDatabase;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorCode};
use serde_json::{Value, json};

use super::{
    CreateNoteParams, EditNoteParams, LinePatch, LineRange, ListNotesParams, NoteTools,
    ReadNoteParams, compute_range_etag, compute_section_etag,
};
use crate::api::notifier::ChangeNotifier;

const NINE_LINES: &str = "L1\nL2\nL3\nL4\nL5\nL6\nL7\nL8\nL9";

async fn setup_tools() -> (Arc<SqliteDatabase>, NoteTools<SqliteDatabase>) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());
    (db, tools)
}

async fn create_note(db: &SqliteDatabase, content: &str) -> String {
    let note = Note {
        id: String::new(),
        title: "Range Note".to_string(),
        content: content.to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        status: context_core::NoteStatus::Active,
        created_at: None,
        updated_at: None,
        archived_at: None,
        deleted_at: None,
    };

    db.notes()
        .create(&note)
        .await
        .expect("Create should succeed")
        .id
}

fn text_json(result: &CallToolResult) -> Value {
    match result.content.first() {
        Some(ContentBlock::Text(text)) => {
            serde_json::from_str(&text.text).expect("tool output should be valid JSON")
        }
        _ => panic!("expected a text content block"),
    }
}

async fn range_read(
    tools: &NoteTools<SqliteDatabase>,
    note_id: &str,
    ranges: &[(usize, usize)],
    format: Option<&str>,
) -> Value {
    let params = ReadNoteParams {
        note_id: note_id.to_string(),
        ranges: Some(
            ranges
                .iter()
                .map(|(start, end)| LineRange {
                    start: *start,
                    end: *end,
                })
                .collect(),
        ),
        format: format.map(str::to_string),
        anchor: None,
    };

    let result = tools
        .note_read(Parameters(params))
        .await
        .expect("range read should succeed");
    text_json(&result)
}

async fn full_etag(tools: &NoteTools<SqliteDatabase>, note_id: &str) -> String {
    let params = ReadNoteParams {
        note_id: note_id.to_string(),
        ranges: None,
        format: Some("json".to_string()),
        anchor: None,
    };

    let result = tools
        .note_read(Parameters(params))
        .await
        .expect("full read should succeed");
    text_json(&result)["etag"]
        .as_str()
        .expect("full read should return an etag")
        .to_string()
}

fn range_etag(response: &Value, index: usize) -> String {
    response["etags"][index]["etag"]
        .as_str()
        .expect("range read should return an etag per range")
        .to_string()
}

fn stale_of(etag: &str) -> String {
    let mut chars: Vec<char> = etag.chars().collect();
    chars[0] = if chars[0] == '0' { '1' } else { '0' };
    chars.into_iter().collect()
}

fn patch(start: usize, end: usize, content: &str, etag: Option<String>) -> LinePatch {
    LinePatch {
        start: Some(start),
        end: Some(end),
        content: content.to_string(),
        etag,
        anchor: None,
    }
}

fn anchor_patch(anchor: &str, content: &str, etag: Option<String>) -> LinePatch {
    LinePatch {
        start: None,
        end: None,
        content: content.to_string(),
        etag,
        anchor: Some(anchor.to_string()),
    }
}

fn edit_params(note_id: &str, etag: Option<String>, patches: Vec<LinePatch>) -> EditNoteParams {
    EditNoteParams {
        note_id: note_id.to_string(),
        etag,
        title: None,
        tags: None,
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
        status: None,
        patches,
    }
}

#[test]
fn compute_range_etag_is_first_four_bytes_of_sha256() {
    use sha2::Digest;

    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, "3\n5\nLine 3\nLine 4\nLine 5".as_bytes());
    let digest = hasher.finalize();
    let expected: String = digest
        .iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect();

    let actual = compute_range_etag(3, 5, "Line 3\nLine 4\nLine 5");

    assert_eq!(
        actual, expected,
        "range etag must hash start, end, and range text"
    );
    assert_eq!(actual.len(), 8, "range etag must be 8 hex characters");
    assert!(
        actual.chars().all(|c| c.is_ascii_hexdigit()),
        "range etag must be hex, got: {actual}"
    );
    assert_eq!(
        actual,
        actual.to_ascii_lowercase(),
        "range etag must be lowercase"
    );
    assert_ne!(
        actual,
        compute_range_etag(3, 6, "Line 3\nLine 4\nLine 5"),
        "changing the end bound must change the etag"
    );
    assert_ne!(
        actual,
        compute_range_etag(3, 5, "Line 3\nLine 4\nCHANGED"),
        "changing the range text must change the etag"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn range_read_sorts_ranges_and_numbers_toon_lines_per_range() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let response = range_read(&tools, &note_id, &[(7, 9), (1, 3)], None).await;

    assert_eq!(response["etag_scope"], "range");
    assert_eq!(
        response["ranges"],
        json!([{"start": 1, "end": 3}, {"start": 7, "end": 9}]),
        "ranges must be returned in ascending order"
    );
    assert_eq!(
        response["content"], "lines[6]{ln,text}:\n  1,L1\n  2,L2\n  3,L3\n  7,L7\n  8,L8\n  9,L9",
        "each range must keep its absolute line numbers"
    );

    let etags = response["etags"]
        .as_array()
        .expect("etags should be an array");
    assert_eq!(etags.len(), 2, "one etag per range");
    assert_eq!(etags[0]["start"], 1);
    assert_eq!(etags[0]["end"], 3);
    assert_eq!(etags[1]["start"], 7);
    assert_eq!(etags[1]["end"], 9);
    assert_eq!(
        range_etag(&response, 0),
        compute_range_etag(1, 3, "L1\nL2\nL3")
    );
    assert_eq!(
        range_etag(&response, 1),
        compute_range_etag(7, 9, "L7\nL8\nL9")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn range_read_json_mode_aligns_etags_with_ranges_and_line_groups() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let response = range_read(&tools, &note_id, &[(7, 9), (1, 3)], Some("json")).await;

    assert_eq!(response["etag_scope"], "range");
    assert_eq!(
        response["ranges"],
        json!([{"start": 1, "end": 3}, {"start": 7, "end": 9}])
    );
    assert_eq!(response["line_groups"], json!(["L1\nL2\nL3", "L7\nL8\nL9"]));
    assert_eq!(
        range_etag(&response, 0),
        compute_range_etag(1, 3, "L1\nL2\nL3")
    );
    assert_eq!(
        range_etag(&response, 1),
        compute_range_etag(7, 9, "L7\nL8\nL9")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn repeated_range_reads_return_the_same_etag() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let first = range_read(&tools, &note_id, &[(2, 4)], None).await;
    let second = range_read(&tools, &note_id, &[(2, 4)], None).await;

    assert_eq!(
        range_etag(&first, 0),
        range_etag(&second, 0),
        "an unmodified range must keep its etag"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn editing_a_disjoint_range_keeps_this_range_etag() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let before = range_read(&tools, &note_id, &[(1, 3)], None).await;
    let before_etag = range_etag(&before, 0);

    let far = range_read(&tools, &note_id, &[(7, 9)], Some("json")).await;
    let far_etag = range_etag(&far, 0);

    let params = edit_params(&note_id, None, vec![patch(7, 9, "NEW", Some(far_etag))]);
    tools
        .note_edit(Parameters(params))
        .await
        .expect("a disjoint range edit must succeed with a range etag");

    let after = range_read(&tools, &note_id, &[(1, 3)], None).await;

    assert_eq!(
        range_etag(&after, 0),
        before_etag,
        "an untouched range must keep its etag after a disjoint edit"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn editing_within_a_range_changes_its_etag() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let before = range_read(&tools, &note_id, &[(2, 4)], None).await;
    let before_etag = range_etag(&before, 0);

    let params = edit_params(
        &note_id,
        None,
        vec![patch(2, 4, "CHANGED", Some(before_etag.clone()))],
    );
    tools
        .note_edit(Parameters(params))
        .await
        .expect("editing within the range must succeed with its etag");

    let after = range_read(&tools, &note_id, &[(2, 4)], None).await;

    assert_ne!(
        range_etag(&after, 0),
        before_etag,
        "an edited range must report a new etag"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn mismatched_patch_etag_is_rejected_and_content_stays_unchanged() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let params = edit_params(
        &note_id,
        None,
        vec![patch(2, 3, "X", Some("deadbeef".to_string()))],
    );
    let error = tools
        .note_edit(Parameters(params))
        .await
        .expect_err("a mismatched patch etag must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("(2, 3)"),
        "error must name the offending range, got: {}",
        error.message
    );

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(
        unchanged.content, NINE_LINES,
        "a rejected edit must not change the note content"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_without_etag_and_without_whole_note_etag_is_rejected() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let params = edit_params(&note_id, None, vec![patch(2, 3, "X", None)]);
    let error = tools
        .note_edit(Parameters(params))
        .await
        .expect_err("a patch without an etag and no whole-note etag must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("(2, 3)"),
        "error must name the offending range, got: {}",
        error.message
    );
    assert!(
        error.message.contains("etag"),
        "error must mention the missing etag, got: {}",
        error.message
    );

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.content, NINE_LINES);
}

#[tokio::test(flavor = "multi_thread")]
async fn whole_note_etag_still_rejects_stale_metadata_edits() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let etag = full_etag(&tools, &note_id).await;
    let mut params = edit_params(&note_id, Some(stale_of(&etag)), vec![]);
    params.title = Some("Renamed".to_string());

    let error = tools
        .note_edit(Parameters(params))
        .await
        .expect_err("a stale whole-note etag must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert_eq!(
        error.message.as_ref(),
        "Note has been modified since last read. Please re-read the note before editing."
    );

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.title, "Range Note");
}

#[tokio::test(flavor = "multi_thread")]
async fn metadata_only_edit_without_an_etag_succeeds() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let mut params = edit_params(&note_id, None, vec![]);
    params.title = Some("New Title".to_string());

    tools
        .note_edit(Parameters(params))
        .await
        .expect("a metadata-only edit without an etag must succeed");

    let updated = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(updated.title, "New Title");
}

#[tokio::test(flavor = "multi_thread")]
async fn metadata_only_edit_succeeds_with_a_current_whole_note_etag() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let etag = full_etag(&tools, &note_id).await;
    let mut params = edit_params(&note_id, Some(etag), vec![]);
    params.title = Some("Renamed".to_string());

    tools
        .note_edit(Parameters(params))
        .await
        .expect("a metadata-only edit with the current etag must succeed");

    let updated = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(updated.title, "Renamed");
}

#[tokio::test(flavor = "multi_thread")]
async fn whole_note_etag_covers_patches_without_range_etags() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let etag = full_etag(&tools, &note_id).await;
    let params = edit_params(&note_id, Some(etag), vec![patch(2, 2, "X", None)]);

    tools
        .note_edit(Parameters(params))
        .await
        .expect("a whole-note etag must cover patches without range etags");

    let updated = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(updated.content, "L1\nX\nL3\nL4\nL5\nL6\nL7\nL8\nL9");
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_out_of_bounds_is_rejected_before_any_write() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let params = edit_params(&note_id, None, vec![patch(8, 20, "X", Some("abc".into()))]);
    let error = tools
        .note_edit(Parameters(params))
        .await
        .expect_err("an out-of-bounds patch must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.content, NINE_LINES);
}

#[test]
fn edit_note_params_deserializes_old_and_new_payloads() {
    let old: EditNoteParams = serde_json::from_str(
        r#"{"note_id":"n1","etag":"abcd1234","patches":[{"start":1,"end":2,"content":"x","etag":"beef1234"}]}"#,
    )
    .expect("the old payload shape must deserialize");
    assert_eq!(old.etag.as_deref(), Some("abcd1234"));
    assert_eq!(old.patches[0].etag.as_deref(), Some("beef1234"));
    assert_eq!(old.patches[0].start, Some(1));
    assert_eq!(old.patches[0].end, Some(2));
    assert_eq!(old.patches[0].anchor, None);

    let new: EditNoteParams =
        serde_json::from_str(r#"{"note_id":"n1","patches":[{"start":1,"end":2,"content":"x"}]}"#)
            .expect("a payload without any etag must deserialize");
    assert_eq!(new.etag, None);
    assert_eq!(new.patches[0].etag, None);

    let anchored: EditNoteParams = serde_json::from_value(json!({
        "note_id": "n1",
        "patches": [{"anchor": "## Foo", "content": "x"}],
    }))
    .expect("a payload with an anchor patch must deserialize");
    assert_eq!(anchored.patches[0].anchor.as_deref(), Some("## Foo"));
    assert_eq!(anchored.patches[0].start, None);
    assert_eq!(anchored.patches[0].end, None);
}

const SECTIONED: &str = "Preamble\n\n## Foo\nFoo body one\nFoo body two\n## Bar\nBar body";
const FOO_SECTION: &str = "## Foo\nFoo body one\nFoo body two";

fn anchor_params(note_id: &str, anchor: &str, format: Option<&str>) -> ReadNoteParams {
    ReadNoteParams {
        note_id: note_id.to_string(),
        ranges: None,
        anchor: Some(anchor.to_string()),
        format: format.map(str::to_string),
    }
}

async fn anchor_read(
    tools: &NoteTools<SqliteDatabase>,
    note_id: &str,
    anchor: &str,
    format: Option<&str>,
) -> Value {
    let result = tools
        .note_read(Parameters(anchor_params(note_id, anchor, format)))
        .await
        .expect("anchor read should succeed");
    text_json(&result)
}

fn section_etag(response: &Value) -> &str {
    response["etags"][0]["etag"]
        .as_str()
        .expect("an anchor read should return the section etag")
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_read_returns_the_resolved_section_in_toon_mode() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let response = anchor_read(&tools, &note_id, "## Foo", None).await;

    assert_eq!(response["note_id"], note_id);
    assert_eq!(response["anchor"], "## Foo");
    assert_eq!(response["resolved_range"], json!({"start": 3, "end": 5}));
    assert_eq!(response["ranges"], json!([{"start": 3, "end": 5}]));
    assert_eq!(response["etag_scope"], "section");
    assert_eq!(
        response["content"], "lines[3]{ln,text}:\n  3,## Foo\n  4,Foo body one\n  5,Foo body two",
        "the content must cover exactly the resolved lines"
    );
    assert_eq!(
        section_etag(&response),
        compute_section_etag(FOO_SECTION),
        "the etag must cover the section text only"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_read_returns_line_groups_in_json_mode() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let response = anchor_read(&tools, &note_id, "## Foo", Some("json")).await;

    assert_eq!(response["line_groups"], json!([FOO_SECTION]));
    assert_eq!(response["resolved_range"], json!({"start": 3, "end": 5}));
    assert_eq!(response["etag_scope"], "section");
    assert_eq!(section_etag(&response), compute_section_etag(FOO_SECTION));
    assert!(
        response.get("content").is_none(),
        "json mode must return line_groups instead of TOON content"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn text_anchor_read_returns_the_single_matching_line() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let response = anchor_read(&tools, &note_id, "Foo body two", None).await;

    assert_eq!(response["resolved_range"], json!({"start": 5, "end": 5}));
    assert_eq!(
        section_etag(&response),
        compute_section_etag("Foo body two")
    );
    assert_eq!(response["content"], "lines[1]{ln,text}:\n  5,Foo body two");
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_read_rejects_anchor_and_ranges_together() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let mut params = anchor_params(&note_id, "## Foo", None);
    params.ranges = Some(vec![LineRange { start: 1, end: 2 }]);

    let error = tools
        .note_read(Parameters(params))
        .await
        .expect_err("anchor and ranges are mutually exclusive");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("anchor") && error.message.contains("ranges"),
        "error must name both parameters, got: {}",
        error.message
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_etag_survives_lines_inserted_above_the_section() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let before = anchor_read(&tools, &note_id, "## Foo", Some("json")).await;
    let before_etag = section_etag(&before).to_string();
    assert_eq!(before["resolved_range"], json!({"start": 3, "end": 5}));

    let etag = full_etag(&tools, &note_id).await;
    let params = edit_params(
        &note_id,
        Some(etag),
        vec![patch(1, 1, "A\nB\nC\nD\nE\nPreamble", None)],
    );
    tools
        .note_edit(Parameters(params))
        .await
        .expect("inserting lines above the section must succeed");

    let after = anchor_read(&tools, &note_id, "## Foo", Some("json")).await;

    assert_eq!(
        after["resolved_range"],
        json!({"start": 8, "end": 10}),
        "resolution must follow the shifted section"
    );
    assert_eq!(
        section_etag(&after),
        before_etag,
        "the section etag must not change when lines are inserted above it"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_read_reports_a_missing_anchor_and_leaves_the_note_unchanged() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let error = tools
        .note_read(Parameters(anchor_params(&note_id, "## Missing", None)))
        .await
        .expect_err("an unmatched anchor must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("## Missing"),
        "error must name the anchor, got: {}",
        error.message
    );
    assert_eq!(
        error.data.expect("error should carry a data payload")["anchor"],
        "## Missing"
    );

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.content, SECTIONED);
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_read_reports_candidate_lines_when_ambiguous() {
    let content = "Alpha one\nAlpha two\n# H";
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, content).await;

    let error = tools
        .note_read(Parameters(anchor_params(&note_id, "Alpha", None)))
        .await
        .expect_err("an ambiguous anchor must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("1, 2"),
        "error must list the candidate lines, got: {}",
        error.message
    );

    let data = error.data.expect("error should carry a data payload");
    assert_eq!(data["anchor"], "Alpha");
    assert_eq!(data["lines"], json!([1, 2]));

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.content, content);
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_patch_applies_at_the_resolved_range() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let read = anchor_read(&tools, &note_id, "## Foo", Some("json")).await;
    let etag = section_etag(&read).to_string();

    let params = edit_params(
        &note_id,
        None,
        vec![anchor_patch("## Foo", "## Foo\nREPLACED", Some(etag))],
    );
    let result = tools
        .note_edit(Parameters(params))
        .await
        .expect("an anchor patch with the section etag must succeed");

    let expected = "Preamble\n\n## Foo\nREPLACED\n## Bar\nBar body";
    assert_eq!(text_json(&result)["content"], expected);

    let stored = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(stored.content, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_patch_follows_lines_inserted_above_the_section() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let before = anchor_read(&tools, &note_id, "## Foo", Some("json")).await;
    let etag = section_etag(&before).to_string();

    let whole = full_etag(&tools, &note_id).await;
    let insert = edit_params(
        &note_id,
        Some(whole),
        vec![patch(1, 1, "A\nB\nC\nD\nE\nPreamble", None)],
    );
    tools
        .note_edit(Parameters(insert))
        .await
        .expect("inserting five lines above the section must succeed");

    let shifted = anchor_read(&tools, &note_id, "## Foo", Some("json")).await;
    assert_eq!(
        shifted["resolved_range"],
        json!({"start": 8, "end": 10}),
        "resolution must move to the shifted section"
    );
    assert_eq!(
        section_etag(&shifted),
        etag,
        "the section etag must not change when lines are inserted above it"
    );

    let params = edit_params(
        &note_id,
        None,
        vec![anchor_patch(
            "## Foo",
            "## Foo\nFoo body one\nFoo body two\nFoo body three",
            Some(etag),
        )],
    );
    tools
        .note_edit(Parameters(params))
        .await
        .expect("an anchor patch must succeed at the shifted location");

    let stored = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(
        stored.content,
        "A\nB\nC\nD\nE\nPreamble\n\n## Foo\nFoo body one\nFoo body two\nFoo body three\n## Bar\nBar body"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_section_etag_is_rejected_and_content_stays_unchanged() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let read = anchor_read(&tools, &note_id, "## Foo", Some("json")).await;
    let stale = section_etag(&read).to_string();

    let whole = full_etag(&tools, &note_id).await;
    let inner_edit = edit_params(
        &note_id,
        Some(whole),
        vec![patch(4, 4, "Foo body CHANGED", None)],
    );
    tools
        .note_edit(Parameters(inner_edit))
        .await
        .expect("editing inside the section must succeed with the whole-note etag");

    let changed = "Preamble\n\n## Foo\nFoo body CHANGED\nFoo body two\n## Bar\nBar body";

    let params = edit_params(
        &note_id,
        None,
        vec![anchor_patch("## Foo", "x", Some(stale))],
    );
    let error = tools
        .note_edit(Parameters(params))
        .await
        .expect_err("a stale section etag must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("(3, 5)"),
        "error must name the resolved range, got: {}",
        error.message
    );
    assert!(
        error.message.contains("Section"),
        "error must name the section, got: {}",
        error.message
    );

    let stored = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(stored.content, changed);
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_patch_without_etag_and_without_whole_note_etag_is_rejected() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let params = edit_params(&note_id, None, vec![anchor_patch("## Foo", "x", None)]);
    let error = tools
        .note_edit(Parameters(params))
        .await
        .expect_err("an anchor patch without any etag must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("(3, 5)"),
        "error must name the resolved range, got: {}",
        error.message
    );
    assert!(
        error.message.contains("etag"),
        "error must mention the missing etag, got: {}",
        error.message
    );

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.content, SECTIONED);
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_patch_reports_an_unresolved_anchor_before_etag_validation() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let params = edit_params(&note_id, None, vec![anchor_patch("## Missing", "x", None)]);
    let error = tools
        .note_edit(Parameters(params))
        .await
        .expect_err("an unmatched anchor must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("## Missing"),
        "error must name the anchor, got: {}",
        error.message
    );
    assert_eq!(
        error.data.expect("error should carry a data payload")["anchor"],
        "## Missing"
    );

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.content, SECTIONED);
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_patch_reports_ambiguous_anchors_with_candidate_lines() {
    let content = "Alpha one\nAlpha two\n# H";
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, content).await;

    let params = edit_params(
        &note_id,
        None,
        vec![anchor_patch("Alpha", "x", Some("deadbeef".to_string()))],
    );
    let error = tools
        .note_edit(Parameters(params))
        .await
        .expect_err("an ambiguous anchor must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("Alpha") && error.message.contains("1, 2"),
        "error must name the anchor and the candidate lines, got: {}",
        error.message
    );

    let data = error.data.expect("error should carry a data payload");
    assert_eq!(data["anchor"], "Alpha");
    assert_eq!(data["lines"], json!([1, 2]));

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.content, content);
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_target_shape_is_validated() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let both_targets = LinePatch {
        start: Some(3),
        end: Some(3),
        content: "x".to_string(),
        etag: None,
        anchor: Some("## Foo".to_string()),
    };
    let error = tools
        .note_edit(Parameters(edit_params(&note_id, None, vec![both_targets])))
        .await
        .expect_err("a patch with both targets must be rejected");
    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.message.contains("Patch 0"),
        "error must name the patch index, got: {}",
        error.message
    );
    assert_eq!(
        error.data.expect("error should carry a data payload")["patch_index"],
        0
    );

    let no_target = LinePatch {
        start: None,
        end: None,
        content: "x".to_string(),
        etag: None,
        anchor: None,
    };
    let error = tools
        .note_edit(Parameters(edit_params(&note_id, None, vec![no_target])))
        .await
        .expect_err("a patch without a target must be rejected");
    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(error.message.contains("Patch 0"));

    let invalid_shapes = [
        (
            "only 'start'",
            LinePatch {
                start: Some(3),
                end: None,
                content: "x".to_string(),
                etag: None,
                anchor: None,
            },
        ),
        (
            "only 'end'",
            LinePatch {
                start: None,
                end: Some(3),
                content: "x".to_string(),
                etag: None,
                anchor: None,
            },
        ),
        (
            "'anchor' plus 'start'",
            LinePatch {
                start: Some(3),
                end: None,
                content: "x".to_string(),
                etag: None,
                anchor: Some("## Foo".to_string()),
            },
        ),
        (
            "'anchor' plus 'end'",
            LinePatch {
                start: None,
                end: Some(3),
                content: "x".to_string(),
                etag: None,
                anchor: Some("## Foo".to_string()),
            },
        ),
    ];

    for (label, invalid) in invalid_shapes {
        let result = tools
            .note_edit(Parameters(edit_params(&note_id, None, vec![invalid])))
            .await;
        let Err(error) = result else {
            panic!("a patch with {label} must be rejected");
        };

        assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
        assert!(
            error.message.contains("Patch 0"),
            "error must name the patch index, got: {}",
            error.message
        );
    }

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.content, SECTIONED);

    let empty_anchor = LinePatch {
        start: Some(4),
        end: Some(4),
        content: "Foo body X".to_string(),
        etag: None,
        anchor: Some(String::new()),
    };
    let whole = full_etag(&tools, &note_id).await;
    tools
        .note_edit(Parameters(edit_params(
            &note_id,
            Some(whole),
            vec![empty_anchor],
        )))
        .await
        .expect("an empty anchor string must count as absent");

    let stored = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(
        stored.content,
        "Preamble\n\n## Foo\nFoo body X\nFoo body two\n## Bar\nBar body"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn overlapping_anchor_patches_are_rejected_and_content_stays_unchanged() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    let read = anchor_read(&tools, &note_id, "## Foo", Some("json")).await;
    let etag = section_etag(&read).to_string();

    let params = edit_params(
        &note_id,
        None,
        vec![
            anchor_patch("## Foo", "one", Some(etag)),
            anchor_patch(
                "Foo body one",
                "two",
                Some(compute_section_etag("Foo body one")),
            ),
        ],
    );
    let error = tools
        .note_edit(Parameters(params))
        .await
        .expect_err("overlapping anchor patches must be rejected");

    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    let data = error.data.expect("error should carry a data payload");
    let message = data["message"]
        .as_str()
        .expect("a DB validation error should carry a message");
    assert!(
        message.contains("Overlapping") && message.contains("(3, 5)") && message.contains("(4, 4)"),
        "error must name the overlapping ranges, got: {message}"
    );

    let unchanged = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(unchanged.content, SECTIONED);
}

const BLANK_TRAILING: &str = "L1\nL2\n\nL4\nL5";
const BLANK_SECTION: &str = "## A\nA body\n\n## B\nB body";

#[tokio::test(flavor = "multi_thread")]
async fn toon_range_read_keeps_a_trailing_blank_line() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, BLANK_TRAILING).await;

    let response = range_read(&tools, &note_id, &[(1, 3)], None).await;

    assert_eq!(
        response["content"], "lines[3]{ln,text}:\n  1,L1\n  2,L2\n  3,",
        "a range ending in a blank line must render one row per line"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn toon_range_read_renders_a_single_blank_line_range() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, BLANK_TRAILING).await;

    let response = range_read(&tools, &note_id, &[(3, 3)], None).await;

    assert_eq!(
        response["content"], "lines[1]{ln,text}:\n  3,",
        "a range naming a single blank line must render one row"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn toon_anchor_read_keeps_trailing_blank_lines_of_the_section() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, BLANK_SECTION).await;

    let response = anchor_read(&tools, &note_id, "## A", None).await;

    assert_eq!(response["resolved_range"], json!({"start": 1, "end": 3}));
    assert_eq!(
        response["content"], "lines[3]{ln,text}:\n  1,## A\n  2,A body\n  3,",
        "an anchor section ending in a blank line must keep that row"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn full_note_read_toon_mode_numbers_every_line() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let params = ReadNoteParams {
        note_id: note_id.clone(),
        ranges: None,
        anchor: None,
        format: None,
    };
    let result = tools
        .note_read(Parameters(params))
        .await
        .expect("full read should succeed");
    let response = text_json(&result);

    assert_eq!(
        response["content"],
        "lines[9]{ln,text}:\n  1,L1\n  2,L2\n  3,L3\n  4,L4\n  5,L5\n  6,L6\n  7,L7\n  8,L8\n  9,L9",
        "a full note read must number every line from 1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn anchor_read_rejects_empty_and_whitespace_only_anchors() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, SECTIONED).await;

    for anchor in ["", "   ", "\t\n"] {
        let error = tools
            .note_read(Parameters(anchor_params(&note_id, anchor, None)))
            .await
            .expect_err("an empty anchor must be rejected as an invalid parameter");

        assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
        assert!(
            error.message.contains("anchor"),
            "error must name the anchor parameter, got: {}",
            error.message
        );

        let data = error.data.expect("error should carry a data payload");
        assert_eq!(data["note_id"], note_id);
        assert_eq!(data["anchor"], anchor);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn note_create_stores_multiline_content_verbatim() {
    let (db, tools) = setup_tools().await;
    let content = "First line\nSecond line\n\nFourth line";

    let params = CreateNoteParams {
        title: "Multi-line Note".to_string(),
        content: content.to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
    };

    let result = tools
        .note_create(Parameters(params))
        .await
        .expect("note_create should succeed");
    let response = text_json(&result);

    assert_eq!(
        response["content"],
        json!(content),
        "note_create must return the multi-line content verbatim"
    );

    let note_id = response["id"]
        .as_str()
        .expect("created note must carry an id");
    let stored = db.notes().get(note_id).await.expect("Get should succeed");

    assert_eq!(
        stored.content, content,
        "stored content must equal the input string byte for byte"
    );
    assert!(
        stored.content.contains('\n'),
        "stored content must keep U+000A characters"
    );
    assert!(
        !stored.content.contains("\\n"),
        "stored content must not contain literal backslash-n sequences, got: {:?}",
        stored.content
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_edit_stores_multiline_replacement_verbatim() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, "Alpha\nBeta\nGamma").await;

    let etag = full_etag(&tools, &note_id).await;
    let params = edit_params(
        &note_id,
        Some(etag),
        vec![patch(2, 2, "First replacement\nSecond replacement", None)],
    );

    tools
        .note_edit(Parameters(params))
        .await
        .expect("a multi-line replacement must succeed");

    let stored = db.notes().get(&note_id).await.expect("Get should succeed");

    assert_eq!(
        stored.content, "Alpha\nFirst replacement\nSecond replacement\nGamma",
        "a multi-line replacement must be stored with real newlines"
    );
    assert!(
        !stored.content.contains("\\n"),
        "stored content must not contain literal backslash-n sequences, got: {:?}",
        stored.content
    );
}

async fn list_with_status(tools: &NoteTools<SqliteDatabase>, status: Option<&str>) -> Value {
    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: None,
        status: status.map(str::to_string),
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .note_list(Parameters(params))
        .await
        .expect("note_list should succeed");
    text_json(&result)
}

async fn list_with_query_and_status(
    tools: &NoteTools<SqliteDatabase>,
    query: &str,
    status: Option<&str>,
) -> Value {
    let params = ListNotesParams {
        query: Some(query.to_string()),
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: None,
        status: status.map(str::to_string),
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .note_list(Parameters(params))
        .await
        .expect("note_list should succeed");
    text_json(&result)
}

async fn edit_status(tools: &NoteTools<SqliteDatabase>, note_id: &str, status: &str) -> Value {
    let etag = full_etag(tools, note_id).await;
    let mut params = edit_params(note_id, Some(etag), vec![]);
    params.status = Some(status.to_string());

    let result = tools
        .note_edit(Parameters(params))
        .await
        .expect("a status-only edit must succeed");
    text_json(&result)
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list_filters_by_status() {
    let (db, tools) = setup_tools().await;
    let active_id = create_note(&db, "Active body").await;
    let archived_id = create_note(&db, "Archived body").await;
    edit_status(&tools, &archived_id, "archived").await;

    let archived = list_with_status(&tools, Some("archived")).await;
    assert_eq!(
        archived["total"], 1,
        "status=archived must return only archived notes"
    );
    assert_eq!(archived["items"][0]["id"], json!(archived_id));
    assert_eq!(archived["items"][0]["status"], "archived");

    let active = list_with_status(&tools, Some("active")).await;
    assert_eq!(
        active["total"], 1,
        "status=active must return only active notes"
    );
    assert_eq!(active["items"][0]["id"], json!(active_id));

    let both = list_with_status(&tools, None).await;
    assert_eq!(
        both["total"], 2,
        "omitting status must return both statuses"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list_combines_query_and_status() {
    let (db, tools) = setup_tools().await;
    let archived_id = create_note(&db, "Deliberately archived body").await;
    create_note(&db, "Unrelated active body").await;
    edit_status(&tools, &archived_id, "archived").await;

    let search_only = list_with_query_and_status(&tools, "deliberately", None).await;
    assert_eq!(
        search_only["total"], 1,
        "query alone must match the archived note"
    );

    let archived = list_with_query_and_status(&tools, "deliberately", Some("archived")).await;
    assert_eq!(
        archived["total"], 1,
        "query with status=archived must return the archived match"
    );
    assert_eq!(archived["items"][0]["id"], json!(archived_id));
    assert_eq!(archived["items"][0]["status"], "archived");

    let active = list_with_query_and_status(&tools, "deliberately", Some("active")).await;
    assert_eq!(
        active["total"], 0,
        "query with status=active must exclude the archived match"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_edit_archives_and_unarchives() {
    let (db, tools) = setup_tools().await;
    let note_id = create_note(&db, NINE_LINES).await;

    let archived = edit_status(&tools, &note_id, "archived").await;
    assert_eq!(archived["status"], "archived");
    assert!(
        !archived["archived_at"].is_null(),
        "archiving must set archived_at in the tool response"
    );

    let stored = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(stored.status, NoteStatus::Archived);
    assert!(
        stored.archived_at.is_some(),
        "archiving must set archived_at in the database"
    );

    let active = edit_status(&tools, &note_id, "active").await;
    assert_eq!(active["status"], "active");
    assert!(
        active["archived_at"].is_null(),
        "unarchiving must clear archived_at in the tool response"
    );

    let stored = db.notes().get(&note_id).await.expect("Get should succeed");
    assert_eq!(stored.status, NoteStatus::Active);
    assert_eq!(
        stored.archived_at, None,
        "unarchiving must clear archived_at"
    );
}
