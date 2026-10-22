//! Tests for SyncRepository import of skill attachments.

use crate::SqliteDatabase;
use base64::prelude::*;
use context_core::{
    Database, HasSkills, HasSync, Skill, SkillAttachment, SkillRepository, SyncRepository,
    write_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_skills_with_attachments_upsert() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create skill with attachments
    let skill_content = r#"---
name: test-skill
description: Test skill with attachments
version: "1.0.0"
---

# Test Skill

This skill has attachments.
"#;

    let script_content = b"#!/bin/bash\necho 'test script'";
    let script_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(script_content);
        hasher.finalize().iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            write!(acc, "{:02x}", b).unwrap();
            acc
        })
    };
    let script_b64 = BASE64_STANDARD.encode(script_content);

    let reference_content = b"# Reference Doc\nSome documentation";
    let reference_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(reference_content);
        hasher.finalize().iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            write!(acc, "{:02x}", b).unwrap();
            acc
        })
    };
    let reference_b64 = BASE64_STANDARD.encode(reference_content);

    let skill = Skill {
        id: "sk001234".to_string(),
        name: "test-skill".to_string(),
        description: "Test skill with attachments".to_string(),
        content: skill_content.to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T10:00:00Z".to_string()),
        updated_at: Some("2024-01-01T10:00:00Z".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };

    let attachments = vec![
        SkillAttachment {
            id: "at001234".to_string(),
            skill_id: "sk001234".to_string(),
            type_: "script".to_string(),
            filename: "run.sh".to_string(),
            content: script_b64.clone(),
            content_hash: script_hash.clone(),
            mime_type: Some("text/x-shellscript".to_string()),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-01T10:00:00Z".to_string()),
            deleted_at: None,
        },
        SkillAttachment {
            id: "at005678".to_string(),
            skill_id: "sk001234".to_string(),
            type_: "reference".to_string(),
            filename: "README.md".to_string(),
            content: reference_b64.clone(),
            content_hash: reference_hash.clone(),
            mime_type: Some("text/markdown".to_string()),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-01T10:00:00Z".to_string()),
            deleted_at: None,
        },
    ];

    // Write to separate JSONL files and import
    write_jsonl(
        &temp_dir.path().join("skills.jsonl"),
        std::slice::from_ref(&skill),
    )
    .unwrap();
    write_jsonl(
        &temp_dir.path().join("skills_attachments.jsonl"),
        &attachments,
    )
    .unwrap();
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify skill created
    let imported_skill = db.skills().get("sk001234").await.unwrap();
    assert_eq!(imported_skill.name, "test-skill");

    // Verify attachments created
    let attachments = db.skills().get_attachments("sk001234").await.unwrap();
    assert_eq!(attachments.len(), 2);

    let script_att = attachments.iter().find(|a| a.filename == "run.sh").unwrap();
    assert_eq!(script_att.type_, "script");
    assert_eq!(script_att.content_hash, script_hash);
    assert_eq!(script_att.content, script_b64);
    assert_eq!(
        script_att.mime_type,
        Some("text/x-shellscript".to_string()),
        "import must preserve the script attachment mime_type"
    );

    let ref_att = attachments
        .iter()
        .find(|a| a.filename == "README.md")
        .unwrap();
    assert_eq!(ref_att.type_, "reference");
    assert_eq!(ref_att.content_hash, reference_hash);
    assert_eq!(
        ref_att.mime_type,
        Some("text/markdown".to_string()),
        "import must preserve the reference attachment mime_type"
    );

    // Test 2: Re-import with changed attachment content (should update)
    let modified_script = b"#!/bin/bash\necho 'modified script'";
    let modified_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(modified_script);
        hasher.finalize().iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            write!(acc, "{:02x}", b).unwrap();
            acc
        })
    };
    let modified_b64 = BASE64_STANDARD.encode(modified_script);

    let updated_attachments_export = vec![
        SkillAttachment {
            id: "at001234".to_string(),
            skill_id: "sk001234".to_string(),
            type_: "script".to_string(),
            filename: "run.sh".to_string(),
            content: modified_b64.clone(),
            content_hash: modified_hash.clone(),
            mime_type: Some("text/plain".to_string()),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-02T10:00:00Z".to_string()),
            deleted_at: None,
        },
        SkillAttachment {
            id: "at005678".to_string(),
            skill_id: "sk001234".to_string(),
            type_: "reference".to_string(),
            filename: "README.md".to_string(),
            content: reference_b64.clone(),
            content_hash: reference_hash.clone(),
            mime_type: Some("text/markdown".to_string()),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-01T10:00:00Z".to_string()),
            deleted_at: None,
        },
    ];

    write_jsonl(
        &temp_dir.path().join("skills.jsonl"),
        std::slice::from_ref(&skill),
    )
    .unwrap();
    write_jsonl(
        &temp_dir.path().join("skills_attachments.jsonl"),
        &updated_attachments_export,
    )
    .unwrap();
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify attachment updated
    let updated_attachments = db.skills().get_attachments("sk001234").await.unwrap();
    assert_eq!(updated_attachments.len(), 2);

    let updated_script = updated_attachments
        .iter()
        .find(|a| a.filename == "run.sh")
        .unwrap();
    assert_eq!(updated_script.content_hash, modified_hash);
    assert_eq!(updated_script.content, modified_b64);
    assert_eq!(
        updated_script.mime_type,
        Some("text/plain".to_string()),
        "LWW update must apply the re-imported mime_type"
    );

    // Reference should be unchanged
    let unchanged_ref = updated_attachments
        .iter()
        .find(|a| a.filename == "README.md")
        .unwrap();
    assert_eq!(unchanged_ref.content_hash, reference_hash);

    // Test 3: Re-import with the script attachment tombstoned. The live import
    // never deletes attachments absent from the file; a removal propagates only
    // as a tombstone whose updated_at is later than the stored row.
    let tombstoned_attachments_export = vec![
        SkillAttachment {
            id: "at001234".to_string(),
            skill_id: "sk001234".to_string(),
            type_: "script".to_string(),
            filename: "run.sh".to_string(),
            content: modified_b64.clone(),
            content_hash: modified_hash.clone(),
            mime_type: Some("text/x-shellscript".to_string()),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-03T10:00:00Z".to_string()),
            deleted_at: Some("2024-01-03T10:00:00Z".to_string()),
        },
        SkillAttachment {
            id: "at005678".to_string(),
            skill_id: "sk001234".to_string(),
            type_: "reference".to_string(),
            filename: "README.md".to_string(),
            content: reference_b64.clone(),
            content_hash: reference_hash.clone(),
            mime_type: Some("text/markdown".to_string()),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-01T10:00:00Z".to_string()),
            deleted_at: None,
        },
    ];

    write_jsonl(
        &temp_dir.path().join("skills.jsonl"),
        std::slice::from_ref(&skill),
    )
    .unwrap();
    write_jsonl(
        &temp_dir.path().join("skills_attachments.jsonl"),
        &tombstoned_attachments_export,
    )
    .unwrap();
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify script attachment is soft-deleted (excluded from the live set)
    let live_filenames: Vec<String> = sqlx::query_scalar(
        "SELECT filename FROM skill_attachment WHERE skill_id = ? AND deleted_at IS NULL",
    )
    .bind("sk001234")
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(live_filenames, vec!["README.md".to_string()]);
}
