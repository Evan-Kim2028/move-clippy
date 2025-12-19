//! Spec-driven exhaustive tests for type-based lints.
//!
//! This test file runs exhaustive tests that prove lint correctness
//! by testing all possible input combinations derived from formal specs.
//!
//! Note: Individual spec test modules (droppable_hot_potato, etc.) define their own
//! `#[path = "support/mod.rs"] mod support;` to avoid duplicate module errors.

#![cfg(feature = "full")]

// Include the droppable_hot_potato tests directly
mod droppable_hot_potato;
