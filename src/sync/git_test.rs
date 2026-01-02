use crate::sync::git::*;
use mockall::predicate::*;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{ExitStatus, Output};

/// Helper to create a mock Output
fn mock_output(code: i32, stdout: &str, stderr: &str) -> Output {
    Output {
        status: ExitStatus::from_raw(code),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[test]
fn test_mock_git_init_success() {
    let mut mock = MockGitOps::new();

    mock.expect_init()
        .with(eq(Path::new("/tmp/test")))
        .times(1)
        .returning(|_| {
            Ok(mock_output(
                0,
                "Initialized empty Git repository in /tmp/test/.git/\n",
                "",
            ))
        });

    let result = mock.init(Path::new("/tmp/test"));
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Initialized"));
}

#[test]
fn test_mock_git_init_failure() {
    let mut mock = MockGitOps::new();

    mock.expect_init()
        .with(eq(Path::new("/tmp/test")))
        .times(1)
        .returning(|_| Err(GitError::GitNotFound));

    let result = mock.init(Path::new("/tmp/test"));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), GitError::GitNotFound));
}

#[test]
fn test_mock_status_clean() {
    let mut mock = MockGitOps::new();

    mock.expect_status_porcelain()
        .with(eq(Path::new("/tmp/test")))
        .times(1)
        .returning(|_| Ok(mock_output(0, "", "")));

    let result = mock.status_porcelain(Path::new("/tmp/test"));
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.stdout.is_empty());
}

#[test]
fn test_mock_status_dirty() {
    let mut mock = MockGitOps::new();

    mock.expect_status_porcelain()
        .with(eq(Path::new("/tmp/test")))
        .times(1)
        .returning(|_| Ok(mock_output(0, " M repos.jsonl\n?? newfile.txt\n", "")));

    let result = mock.status_porcelain(Path::new("/tmp/test"));
    assert!(result.is_ok());

    let output = result.unwrap();
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(status.contains("M repos.jsonl"));
    assert!(status.contains("?? newfile.txt"));
}

#[test]
fn test_mock_commit_success() {
    let mut mock = MockGitOps::new();

    mock.expect_commit()
        .with(eq(Path::new("/tmp/test")), eq("Export data"))
        .times(1)
        .returning(|_, _| {
            Ok(mock_output(
                0,
                "[main abc1234] Export data\n 5 files changed, 42 insertions(+)\n",
                "",
            ))
        });

    let result = mock.commit(Path::new("/tmp/test"), "Export data");
    assert!(result.is_ok());
}

#[test]
fn test_mock_pull_up_to_date() {
    let mut mock = MockGitOps::new();

    mock.expect_pull()
        .with(eq(Path::new("/tmp/test")), eq("origin"), eq("main"))
        .times(1)
        .returning(|_, _, _| Ok(mock_output(0, "Already up to date.\n", "")));

    let result = mock.pull(Path::new("/tmp/test"), "origin", "main");
    assert!(result.is_ok());

    let output = result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Already up to date"));
}

#[test]
fn test_mock_push_success() {
    let mut mock = MockGitOps::new();

    mock.expect_push()
        .with(eq(Path::new("/tmp/test")), eq("origin"), eq("main"))
        .times(1)
        .returning(|_, _, _| {
            Ok(mock_output(
                0,
                "",
                "To https://github.com/user/repo.git\n   abc1234..def5678  main -> main\n",
            ))
        });

    let result = mock.push(Path::new("/tmp/test"), "origin", "main");
    assert!(result.is_ok());
}

#[test]
fn test_mock_network_error() {
    let mut mock = MockGitOps::new();

    mock.expect_pull()
        .with(eq(Path::new("/tmp/test")), eq("origin"), eq("main"))
        .times(1)
        .returning(|_, _, _| {
            Err(GitError::NonZeroExit {
                code: 128,
                output: "fatal: unable to access 'https://...': Could not resolve host\n"
                    .to_string(),
            })
        });

    let result = mock.pull(Path::new("/tmp/test"), "origin", "main");
    assert!(result.is_err());

    if let Err(GitError::NonZeroExit { code, output }) = result {
        assert_eq!(code, 128);
        assert!(output.contains("Could not resolve host"));
    } else {
        panic!("Expected NonZeroExit error");
    }
}
