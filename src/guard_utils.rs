//! TODO: Guard/dominance utilities for CFG-aware lints.
//!
//! This module is intentionally a stub for now.
//!
//! Goal:
//! - Provide reusable helpers to identify validation guards (e.g., non-zero checks)
//!   and to reason about whether a guard dominates a sink in the CFG.
//!
//! Planned helpers:
//! - Recognize common `assert!` / `abort` guard shapes in typed AST and/or AbsInt IR.
//! - Provide a small API used by `unchecked_division_v2`, `phantom_capability`,
//!   and future access-control lints.
