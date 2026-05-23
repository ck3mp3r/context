//! Common utilities for handling command output

use std::process::Output;

/// Format command error with clean stderr (ANSI codes stripped) and context
pub fn format_command_error(command_name: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let clean = strip_ansi_escapes::strip_str(&stderr);

    format!(
        "{} failed (exit code: {:?}):\n\n{}",
        command_name,
        output.status.code(),
        clean
    )
}

/// Get clean stderr without ANSI codes
pub fn clean_stderr(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    strip_ansi_escapes::strip_str(&stderr)
}

/// Get clean stdout without ANSI codes
pub fn clean_stdout(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    strip_ansi_escapes::strip_str(&stdout)
}
