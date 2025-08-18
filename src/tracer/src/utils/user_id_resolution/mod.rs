//! User ID Resolution Module
//! 
//! This module provides comprehensive user ID resolution with multiple fallback strategies,
//! robust error handling, and detailed Sentry instrumentation for monitoring and debugging.
//! 
//! ## Features
//! - Multi-strategy resolution (CLI args, env vars, shell configs, system username)
//! - Shell configuration file reading (.zshrc, .bashrc, etc.)
//! - Comprehensive Sentry error reporting and monitoring
//! - Functional programming approach with pure functions
//! - Extensive test coverage

mod resolver;
mod sentry_context;

// Re-export the main functions and types
pub use resolver::resolve_user_id_robust;
pub use sentry_context::{UserIdSentryReporter, create_reporter_with_context};
