pub mod config;
pub mod error;
pub mod github;
pub mod models;
pub mod repo_store;
pub mod ui;

// Make Slint-generated types available to all modules in this crate.
slint::include_modules!();
