//! AES conformance against measured TOS behaviour.
//!
//! See `aes_reference/harness.rs` for what these tests cover, which `global`
//! fields are deliberately not compared and why. The case module is generated
//! by `tosdump/host/mkrusttests`; do not edit it by hand.
//!
//! Both files live in the `aes_reference/` subdirectory so cargo does not pick
//! them up as integration-test targets of their own.

#[path = "aes_reference/harness.rs"]
mod harness;

#[path = "aes_reference/cases_r1.rs"]
mod cases_r1;
