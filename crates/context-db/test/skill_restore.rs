use crate::SqliteDatabase;
use crate::helpers::{read_deleted_at, read_updated_at};
use context_core::{Database, HasSkills, Skill, SkillRepository};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

fn make_skill(id: &str, name: &str) -> Skill {
    Skill {
        id: id.to_string(),
        name: name.to_string(),
        description: "Test skill".to_string(),
        content: format!(
            "---\nname: {}\ndescription: Test skill\n---\n\n# Test\n",
            name
        ),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_skill_clears_deleted_at() {
    let db = setup_db().await;
    let skills = db.skills();

    skills
        .create(&make_skill("sklrstr1", "restore-me"))
        .await
        .expect("Create should succeed");
    skills
        .delete("sklrstr1")
        .await
        .expect("Delete should succeed");

    skills
        .restore("sklrstr1")
        .await
        .expect("Restore should succeed");

    assert!(
        read_deleted_at(db.pool(), "skill", "sklrstr1")
            .await
            .is_none(),
        "restore should clear deleted_at"
    );
    assert_ne!(
        read_updated_at(db.pool(), "skill", "sklrstr1").await,
        Some("2025-01-01 00:00:00".to_string()),
        "restore should bump updated_at"
    );
    skills
        .get("sklrstr1")
        .await
        .expect("restored skill should be readable");
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_nonexistent_skill_returns_not_found() {
    let db = setup_db().await;

    let result = db.skills().restore("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "restoring a missing skill should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_live_skill_is_noop() {
    let db = setup_db().await;
    let skills = db.skills();

    skills
        .create(&make_skill("skllive1", "live-skill"))
        .await
        .expect("Create should succeed");

    skills
        .restore("skllive1")
        .await
        .expect("Restoring a live skill should be a no-op");

    assert!(
        read_deleted_at(db.pool(), "skill", "skllive1")
            .await
            .is_none(),
        "deleted_at should stay NULL"
    );
    assert_eq!(
        read_updated_at(db.pool(), "skill", "skllive1").await,
        Some("2025-01-01 00:00:00".to_string()),
        "a no-op restore must not bump updated_at"
    );
}
