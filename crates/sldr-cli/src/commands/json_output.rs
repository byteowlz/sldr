//! JSON output helpers for agent-friendly CLI commands
//!
//! This module provides consistent JSON output formatting for commands
//! that support the `--json` flag, including error handling.

use serde::Serialize;
use std::process::ExitCode;

/// Standard JSON response wrapper for agent-friendly commands
#[derive(Serialize)]
pub struct JsonResponse<T: Serialize> {
    /// Whether the operation succeeded
    pub success: bool,

    /// Whether this was a dry run (no changes made)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,

    /// The result data (present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    /// Error message (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Detailed error cause (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl<T: Serialize> JsonResponse<T> {
    /// Create a success response
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            dry_run: None,
            data: Some(data),
            error: None,
            cause: None,
        }
    }

    /// Create a success response for a dry run
    pub fn success_dry_run(data: T) -> Self {
        Self {
            success: true,
            dry_run: Some(true),
            data: Some(data),
            error: None,
            cause: None,
        }
    }

    /// Create an error response
    pub fn error(message: impl Into<String>, cause: Option<String>) -> Self {
        Self {
            success: false,
            dry_run: None,
            data: None,
            error: Some(message.into()),
            cause,
        }
    }

    /// Print the JSON response to stdout
    pub fn print(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            println!("{json}");
        }
    }
}

/// Exit codes for CLI commands
/// - 0: Success
/// - 1: Partial failure (some operations failed)
/// - 2: Complete failure (command could not execute)
pub struct ExitCodes;

impl ExitCodes {
    pub const SUCCESS: ExitCode = ExitCode::SUCCESS;
    pub const PARTIAL_FAILURE: ExitCode = ExitCode::FAILURE; // Use FAILURE for now
    pub const FAILURE: ExitCode = ExitCode::FAILURE;
}

/// Helper to run a command with JSON error handling
///
/// If `json_output` is true and the command fails, the error is printed as JSON.
/// Returns the appropriate exit code.
pub fn run_with_json_error_handling<F, T>(json_output: bool, f: F) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<T>,
    T: Serialize,
{
    match f() {
        Ok(_) => Ok(()),
        Err(e) => {
            if json_output {
                let cause = e.source().map(|s| s.to_string());
                let response: JsonResponse<()> = JsonResponse::error(e.to_string(), cause);
                response.print();
                // Return Ok so the error isn't printed twice
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}
