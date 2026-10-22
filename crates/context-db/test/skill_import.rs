//! Tests for skill import source persistence.

use crate::SqliteDatabase;
use context_core::{Database, HasSkills, SkillRepository};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in_memory database");
    db.migrate().expect("Migration should succeed");
    db
}

fn write_skill_md(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).expect("skill directory should be created");
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: src-test\ndescription: d\n---\n# Body\n",
    )
    .expect("SKILL.md should be written");
}

#[tokio::test(flavor = "multi_thread")]
async fn import_skill_stores_source_without_subpath() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().expect("temp dir should be created");
    write_skill_md(temp_dir.path());

    let source = temp_dir
        .path()
        .to_str()
        .expect("temp path should be valid UTF-8")
        .to_string();

    let imported = context_skills::import_skill(&db, &source, None, None, None, false)
        .await
        .expect("import should succeed");

    let stored = db
        .skills()
        .get(&imported.id)
        .await
        .expect("stored skill should exist");
    assert_eq!(
        stored.source,
        Some(source),
        "import must store the source argument"
    );
    assert_eq!(
        stored.source_subpath, None,
        "an import without a subpath must store NULL"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_skill_stores_source_subpath() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().expect("temp dir should be created");
    write_skill_md(&temp_dir.path().join("skills").join("nested"));

    let source = temp_dir
        .path()
        .to_str()
        .expect("temp path should be valid UTF-8")
        .to_string();

    let imported =
        context_skills::import_skill(&db, &source, Some("skills/nested"), None, None, false)
            .await
            .expect("import should succeed");

    let stored = db
        .skills()
        .get(&imported.id)
        .await
        .expect("stored skill should exist");
    assert_eq!(stored.source, Some(source));
    assert_eq!(
        stored.source_subpath,
        Some("skills/nested".to_string()),
        "import must store the subpath argument"
    );
}
