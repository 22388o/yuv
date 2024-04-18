//! Nakamoto's client library.
#![allow(clippy::inconsistent_struct_constructor)]
#![allow(clippy::type_complexity)]
mod controller;
pub use controller::*;
mod error;
pub mod peer;

pub mod handle;
mod service;
pub(crate) mod stream;
