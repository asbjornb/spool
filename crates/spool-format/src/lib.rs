//! # spool-format
//!
//! Core types and serialization for the Spool agent session format.
//!
//! Spool is a JSONL format for recording AI agent sessions. This crate provides:
//! - Type definitions for all entry types
//! - Serialization/deserialization
//! - Validation
//! - Reading and writing `.spool` files
//!
//! ## Example
//!
//! ```rust
//! use spool_format::{SpoolFile, Entry, SessionEntry};
//!
//! // Read a spool file
//! let file = SpoolFile::from_path("session.spool")?;
//!
//! // Iterate over entries
//! for entry in file.entries() {
//!     match entry {
//!         Entry::Prompt(p) => println!("User: {}", p.content),
//!         Entry::Response(r) => println!("Agent: {}", r.content),
//!         _ => {}
//!     }
//! }
//! ```

mod entry;
mod error;
mod file;
mod redaction;
mod validation;

pub use entry::*;
pub use error::*;
pub use file::*;
pub use redaction::*;
pub use validation::*;

/// The current version of the Spool format
pub const FORMAT_VERSION: &str = "1.0";

/// MIME type for Spool files
pub const MIME_TYPE: &str = "application/vnd.spool+jsonl";

/// File extension for Spool files
pub const FILE_EXTENSION: &str = "spool";
