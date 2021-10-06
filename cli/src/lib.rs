//! Gistit library

#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
// This lint causes clippy to yell on `argh` expanded macro
#![allow(clippy::default_trait_access)]
// Test env should be chill
#![cfg_attr(
    test,
    allow(
        unused,
        clippy::all,
        clippy::pedantic,
        clippy::nursery,
        clippy::dbg_macro,
        clippy::unwrap_used,
        clippy::missing_docs_in_private_items,
    )
)]

pub mod cli;
pub mod dispatch;
pub mod send;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unsuported file format")]
    UnsuportedFile { message: String },
    #[error("failed to read file")]
    Read(#[from] std::io::Error),
    #[error("failed to parse command arguments")]
    Argument,
    #[error("invalid addons setup")]
    InvalidAddons { message: String },
}

pub type Result<T> = std::result::Result<T, Error>;
