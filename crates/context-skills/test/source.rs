use super::*;

#[test]
fn test_parse_git_https_simple() {
    let result = parse_source("git+https://github.com/user/repo");
    assert!(result.is_ok());
    match result.unwrap() {
        SourceType::GitHttps { repo_url, path } => {
            assert_eq!(repo_url, "git+https://github.com/user/repo");
            assert_eq!(path, None);
        }
        _ => panic!("Expected GitHttps variant"),
    }
}

#[test]
fn test_parse_git_https_with_path() {
    let result = parse_source("git+https://github.com/user/repo/skills/rust");
    assert!(result.is_ok());
    match result.unwrap() {
        SourceType::GitHttps { repo_url, path } => {
            assert_eq!(repo_url, "git+https://github.com/user/repo");
            assert_eq!(path, Some("skills/rust".to_string()));
        }
        _ => panic!("Expected GitHttps variant"),
    }
}

#[test]
fn test_parse_git_ssh() {
    let result = parse_source("git+ssh://git@github.com/user/repo");
    assert!(result.is_ok());
    match result.unwrap() {
        SourceType::GitSsh { repo_url, path } => {
            assert_eq!(repo_url, "git+ssh://git@github.com/user/repo");
            assert_eq!(path, None);
        }
        _ => panic!("Expected GitSsh variant"),
    }
}

#[test]
fn test_parse_file_uri() {
    let result = parse_source("file:///tmp/skill");
    assert!(result.is_ok());
    match result.unwrap() {
        SourceType::LocalPath { path } => {
            assert_eq!(path, PathBuf::from("/tmp/skill"));
        }
        _ => panic!("Expected LocalPath variant"),
    }
}

#[test]
fn test_parse_local_path() {
    // Create a temp file to test with
    let temp = std::env::temp_dir();
    let result = parse_source(temp.to_str().unwrap());
    assert!(result.is_ok());
    match result.unwrap() {
        SourceType::LocalPath { path } => {
            assert_eq!(path, temp);
        }
        _ => panic!("Expected LocalPath variant"),
    }
}

#[test]
fn test_parse_invalid_git_url() {
    let result = parse_source("git+https://github.com/user");
    assert!(result.is_err());
}

#[test]
fn test_parse_nonexistent_path() {
    let result = parse_source("/this/path/does/not/exist");
    assert!(result.is_err());
}

#[test]
fn test_parse_unsupported_protocol() {
    let result = parse_source("https://example.com/skill.zip");
    assert!(result.is_err());
}

#[test]
fn test_fetch_local_path_valid() {
    let temp = std::env::temp_dir();
    let source_type = SourceType::LocalPath { path: temp.clone() };
    let result = fetch_source(source_type);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), temp);
}

#[test]
fn test_fetch_local_path_nonexistent() {
    let path = PathBuf::from("/this/does/not/exist");
    let source_type = SourceType::LocalPath { path };
    let result = fetch_source(source_type);
    assert!(result.is_err());
    match result.unwrap_err() {
        SourceError::InvalidPath(msg) => assert!(msg.contains("does not exist")),
        _ => panic!("Expected InvalidPath error"),
    }
}

#[test]
fn test_fetch_local_path_file_not_dir() {
    // Create a temp file (not directory)
    let temp_file = std::env::temp_dir().join(format!("test-file-{}", std::process::id()));
    std::fs::write(&temp_file, "test").unwrap();

    let source_type = SourceType::LocalPath {
        path: temp_file.clone(),
    };
    let result = fetch_source(source_type);

    // Cleanup
    std::fs::remove_file(&temp_file).ok();

    assert!(result.is_err());
    match result.unwrap_err() {
        SourceError::InvalidPath(msg) => assert!(msg.contains("not a directory")),
        _ => panic!("Expected InvalidPath error"),
    }
}
