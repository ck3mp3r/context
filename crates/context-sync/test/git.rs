use crate::git::*;
use mockall::predicate::*;
use serial_test::serial;
use std::env;
use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, ExitStatus, Output};
use tempfile::TempDir;

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

/// Set a repo-local git config key in the given repository.
fn set_config(dir: &Path, key: &str, value: &str) {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["config", key, value])
        .output()
        .expect("git config should run");

    assert!(
        output.status.success(),
        "git config {key} should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Count the commits reachable from HEAD in the given repository.
fn rev_list_count(dir: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .expect("git rev-list should run");

    assert!(
        output.status.success(),
        "git rev-list --count HEAD should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn test_real_git_commit_with_hostile_repo_local_signing() {
    let repo = TempDir::new().expect("test temp dir should be created");
    let path = repo.path();
    let git = RealGit::new();
    git.init(path).expect("git init should succeed");

    set_config(path, "user.name", "C5t Test");
    set_config(path, "user.email", "c5t-test@example.com");
    set_config(path, "commit.gpgsign", "true");
    set_config(path, "gpg.format", "ssh");
    set_config(
        path,
        "user.signingkey",
        &path.join("nonexistent-key").to_string_lossy(),
    );

    fs::write(path.join("data.jsonl"), "{\"id\":\"test0000\"}\n")
        .expect("data.jsonl should be written");

    git.add_files(path, &[".".to_string()])
        .expect("git add should succeed");
    git.commit(path, "test")
        .expect("commit should succeed when signing is neutralized");

    assert_eq!(
        rev_list_count(path),
        "1",
        "commit should create exactly one commit"
    );
}

#[test]
#[serial]
fn test_real_git_commit_with_hostile_global_signing() {
    let repo = TempDir::new().expect("test temp dir should be created");
    let path = repo.path();
    let git = RealGit::new();
    git.init(path).expect("git init should succeed");

    let config_dir = TempDir::new().expect("config temp dir should be created");
    let global_config = config_dir.path().join("gitconfig");
    let signing_key = config_dir.path().join("nonexistent-key");
    fs::write(
        &global_config,
        format!(
            "[user]\n\tname = C5t Test\n\temail = c5t-test@example.com\n\tsigningkey = {}\n[commit]\n\tgpgsign = true\n[gpg]\n\tformat = ssh\n",
            signing_key.display()
        ),
    )
    .expect("hostile global config should be written");

    fs::write(path.join("data.jsonl"), "{\"id\":\"test0000\"}\n")
        .expect("data.jsonl should be written");

    unsafe { env::set_var("GIT_CONFIG_GLOBAL", &global_config) };
    let added = git.add_files(path, &[".".to_string()]);
    let committed = git.commit(path, "test");
    unsafe { env::remove_var("GIT_CONFIG_GLOBAL") };

    added.expect("git add should succeed");
    committed.expect("commit should succeed when signing is neutralized");

    assert_eq!(
        rev_list_count(path),
        "1",
        "commit should create exactly one commit"
    );
}
