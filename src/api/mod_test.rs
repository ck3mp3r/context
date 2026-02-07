use super::Config;
use crate::sync::get_data_dir;
use serial_test::serial;
use std::env;
use std::path::PathBuf;

#[test]
fn test_config_default_skills_dir() {
    // Default should be {data_dir}/skills
    let config = Config::default();
    let expected = get_data_dir().join("skills");
    assert_eq!(config.skills_dir, expected);
}

#[test]
#[serial]
fn test_config_new_respects_env_var() {
    // Config::new() should read C5T_SKILLS_DIR env var
    let custom_dir = "/tmp/new-skills-test1";
    unsafe {
        env::set_var("C5T_SKILLS_DIR", custom_dir);
    }

    let config = Config::new();
    assert_eq!(config.skills_dir, PathBuf::from(custom_dir));

    // Cleanup
    unsafe {
        env::remove_var("C5T_SKILLS_DIR");
    }
}

#[test]
#[serial]
fn test_config_builder_overrides_env_var() {
    // Set env var
    unsafe {
        env::set_var("C5T_SKILLS_DIR", "/tmp/env-skills-test2");
    }

    // Builder should override env var
    let custom_dir = PathBuf::from("/tmp/builder-skills-test2");
    let config = Config::default().with_skills_dir(custom_dir.clone());

    assert_eq!(config.skills_dir, custom_dir);

    // Cleanup
    unsafe {
        env::remove_var("C5T_SKILLS_DIR");
    }
}

#[test]
#[serial]
fn test_config_precedence_cli_over_env() {
    // Precedence: CLI flag > env var > default
    unsafe {
        env::set_var("C5T_SKILLS_DIR", "/tmp/env-precedence-test3");
    }

    let cli_dir = PathBuf::from("/tmp/cli-skills-test3");
    let config = Config::new().with_skills_dir(cli_dir.clone());

    assert_eq!(
        config.skills_dir, cli_dir,
        "CLI flag should override env var"
    );

    // Cleanup
    unsafe {
        env::remove_var("C5T_SKILLS_DIR");
    }
}
