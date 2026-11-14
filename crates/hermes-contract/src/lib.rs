//! The Rust side of the Hermes desktop IPC contract.
//!
//! This crate is the part of the Electron main process that does not need a
//! window: the channel inventory (generated from `apps/desktop/electron`), the
//! connection descriptor the renderer dials, the HTTP transport policy, and the
//! port status of every channel.

pub mod channels;
pub mod discovery;
pub mod native_gateway;
pub mod registry;
pub mod scope;
pub mod transport;
pub mod types;

pub use registry::{ChannelStatus, PortLedger};
