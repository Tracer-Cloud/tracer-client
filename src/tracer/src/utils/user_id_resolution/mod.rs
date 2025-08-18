//! User ID Resolution Module
//!
//! This module provides comprehensive user ID resolution with multiple fallback strategies,
//! shell configuration file reading, and detailed Sentry instrumentation for monitoring.
//!
//! ## Resolution Strategies
//! 1. **CLI Arguments**: Direct user_id parameter from command line
//! 2. **Environment Variables**: TRACER_USER_ID environment variable
//! 3. **Shell Configuration Files**: Reads .zshrc, .bashrc, .zprofile, .bash_profile, .profile
//! 4. **System Username Fallback**: Uses USER environment variable as last resort
//!
//! ## Features
//! - Multi-strategy resolution with comprehensive fallbacks
//! - Shell configuration file parsing (matches tracer-installer format)
//! - Detailed Sentry error reporting and monitoring for every failure scenario
//! - Functional programming approach with pure functions
//! - Extensive test coverage and error handling

mod error_reporter;
mod resolver;
mod sentry_context;
mod shell_config_reader;
mod shell_file_parser;

// Re-export the main functions and types
pub use resolver::extract_user_id;
pub use sentry_context::{create_reporter_with_context, UserIdSentryReporter};
