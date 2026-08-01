use crate::cli::utils::*;

#[test]
fn test_truncate_with_ellipsis_short_string() {
    let result = truncate_with_ellipsis("hello", 10);
    assert_eq!(result, "hello");
}

#[test]
fn test_truncate_with_ellipsis_exact_length() {
    let result = truncate_with_ellipsis("hello", 5);
    assert_eq!(result, "hello");
}

#[test]
fn test_truncate_with_ellipsis_long_string() {
    let result = truncate_with_ellipsis("hello world this is a long string", 10);
    assert_eq!(result, "hello w...");
}

#[test]
fn test_truncate_with_ellipsis_unicode() {
    let result = truncate_with_ellipsis("hello 世界", 8);
    assert_eq!(result, "hello 世界"); // 8 chars exactly, no truncation

    // Test truncation of unicode - max=7 means result is 7 chars: "hell..."
    let result2 = truncate_with_ellipsis("hello 世界", 7);
    assert_eq!(result2, "hell...");
}

#[test]
fn test_format_tags_none() {
    let result = format_tags(None);
    assert_eq!(result, "-");
}

#[test]
fn test_format_tags_empty() {
    let tags = vec![];
    let result = format_tags(Some(&tags));
    assert_eq!(result, "-");
}

#[test]
fn test_format_tags_single() {
    let tags = vec!["tag1".to_string()];
    let result = format_tags(Some(&tags));
    assert_eq!(result, "tag1");
}

#[test]
fn test_format_tags_multiple() {
    let tags = vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()];
    let result = format_tags(Some(&tags));
    assert_eq!(result, "tag1, tag2, tag3");
}

#[test]
fn test_parse_tags_none() {
    let result = parse_tags(None);
    assert_eq!(result, None);
}

#[test]
fn test_parse_tags_single() {
    let result = parse_tags(Some("tag1"));
    assert_eq!(result, Some(vec!["tag1".to_string()]));
}

#[test]
fn test_parse_tags_multiple() {
    let result = parse_tags(Some("tag1,tag2,tag3"));
    assert_eq!(
        result,
        Some(vec![
            "tag1".to_string(),
            "tag2".to_string(),
            "tag3".to_string()
        ])
    );
}

#[test]
fn test_parse_tags_with_whitespace() {
    let result = parse_tags(Some("tag1, tag2 , tag3"));
    assert_eq!(
        result,
        Some(vec![
            "tag1".to_string(),
            "tag2".to_string(),
            "tag3".to_string()
        ])
    );
}

#[test]
fn test_apply_table_style() {
    use tabled::builder::Builder;

    let mut builder = Builder::default();
    builder.push_record(["Name", "Value"]);
    builder.push_record(["Test", "123"]);

    let mut table = builder.build();
    apply_table_style(&mut table);

    let output = table.to_string();
    // Rounded style uses ╭─╮│╰─╯ characters
    assert!(output.contains("╭"), "Table should use rounded style");
}
