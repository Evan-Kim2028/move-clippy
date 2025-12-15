//! TODO: Centralized, type-grounded framework catalog.
//!
//! This module is intentionally a stub for now.
//!
//! Goal:
//! - Replace ad-hoc module/function string matching spread across lints
//!   with a small, well-tested catalog of recognized Sui framework sinks/sources.
//!
//! Planned helpers:
//! - `is_event_emit_call(call: &T::ModuleCall) -> bool`
//! - `is_transfer_share_call(call: &T::ModuleCall) -> ShareKind`
//! - `is_coin_split_call(call: &T::ModuleCall) -> bool`
//! - `is_coin_take_call(call: &T::ModuleCall) -> bool`
//!
//! Notes:
//! - Prefer matching on fully-qualified module identity where available.
//! - Avoid name-based heuristics; keep allowlists explicit.
