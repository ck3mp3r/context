use super::*;

#[test]
fn test_generate_skill_id_length_constraint() {
    // Test various skill names to ensure all IDs are exactly 8 characters
    let test_names = vec![
        "rust-async-patterns",
        "nushell-testing",
        "a",
        "test",
        "very-long-skill-name-with-many-characters",
        "🦀 Rust Skill",
        "",
        "UPPERCASE",
        "lowercase",
        "123-numbers",
    ];

    for name in test_names {
        let id = generate_skill_id(name);
        assert_eq!(
            id.len(),
            8,
            "Skill ID for '{}' should be exactly 8 characters, got '{}' with length {}",
            name,
            id,
            id.len()
        );

        // Also verify it's valid hexadecimal
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "Skill ID for '{}' should contain only hex characters, got '{}'",
            name,
            id
        );
    }
}

#[test]
fn test_generate_skill_id_deterministic() {
    // Same name should always produce the same ID
    let name = "rust-async-patterns";
    let id1 = generate_skill_id(name);
    let id2 = generate_skill_id(name);

    assert_eq!(id1, id2, "Same skill name should generate the same ID");
    assert_eq!(id1.len(), 8, "ID should be 8 characters");
}

#[test]
fn test_generate_skill_id_uniqueness() {
    // Different names should produce different IDs
    let name1 = "skill-one";
    let name2 = "skill-two";

    let id1 = generate_skill_id(name1);
    let id2 = generate_skill_id(name2);

    assert_ne!(
        id1, id2,
        "Different skill names should generate different IDs"
    );
    assert_eq!(id1.len(), 8, "ID1 should be 8 characters");
    assert_eq!(id2.len(), 8, "ID2 should be 8 characters");
}
