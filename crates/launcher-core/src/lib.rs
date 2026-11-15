//! Core logic of the PRVDS Game Launcher.
//!
//! Everything in here is plain Rust and independent of the GUI, so it can be
//! unit tested on any OS. Windows specific bits (registry, credential store)
//! sit behind `cfg(windows)`.

pub mod artwork;
pub mod boards;
pub mod connectors;
pub mod desktop_icons;
pub mod error;
pub mod importer;
pub mod library;
pub mod locate;
pub mod model;
pub mod playtime;
pub mod query;
pub mod scanner;
pub mod secrets;
pub mod util;

pub use error::{Error, Result};
