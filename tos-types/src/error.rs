//! TOS GEMDOS / BIOS / XBIOS error codes.
//!
//! Source: extracted from EmuTOS @ `1a7484ab` (`include/biosbind.h`,
//! `include/gemerror.h`). Values are bit-exact with TOS so binary
//! programs see the same numbers.
//!
//! All error codes are negative; `E_OK` is zero. They are returned in
//! the AArch64 PCS return register as `i32` so callers can distinguish
//! "negative = error" via sign without truncation surprises.

#![allow(missing_docs)]

use crate::Long;

/// Operation succeeded. Many TOS calls return non-zero on success
/// (e.g. handles, byte counts), so `E_OK` only applies where the spec
/// explicitly defines a "no error" return value.
pub const E_OK: Long = 0;

// Generic / system errors (`include/gemerror.h`)
pub const ERROR:    Long = -1;   // Generic error
pub const EDRVNR:   Long = -2;   // Drive not ready
pub const EUNCMD:   Long = -3;   // Unknown command
pub const E_CRC:    Long = -4;   // CRC error
pub const EBADRQ:   Long = -5;   // Bad request
pub const E_SEEK:   Long = -6;   // Seek error
pub const EMEDIA:   Long = -7;   // Unknown media
pub const ESECNF:   Long = -8;   // Sector not found
pub const EPAPER:   Long = -9;   // Out of paper
pub const EWRITF:   Long = -10;  // Write fault
pub const EREADF:   Long = -11;  // Read fault
pub const EWRPRO:   Long = -13;  // Write protected
pub const E_CHNG:   Long = -14;  // Media change
pub const EUNDEV:   Long = -15;  // Unknown device
pub const EBADSF:   Long = -16;  // Bad sectors on format
pub const EOTHER:   Long = -17;  // Insert other disk

// GEMDOS-specific errors
pub const EINVFN:   Long = -32;  // Invalid function number
pub const EFILNF:   Long = -33;  // File not found
pub const EPTHNF:   Long = -34;  // Path not found
pub const ENHNDL:   Long = -35;  // No more handles
pub const EACCDN:   Long = -36;  // Access denied
pub const EIHNDL:   Long = -37;  // Invalid handle
pub const ENSMEM:   Long = -39;  // Insufficient memory
pub const EIMBA:    Long = -40;  // Invalid memory block address
pub const EDRIVE:   Long = -46;  // Invalid drive
pub const ENSAME:   Long = -48;  // Cross-device rename
pub const ENMFIL:   Long = -49;  // No more files
pub const ELOCKED:  Long = -58;  // Record is locked
pub const ENSLOCK:  Long = -59;  // Invalid lock removal request
pub const ERANGE:   Long = -64;  // Seek out of range
pub const EINTRN:   Long = -65;  // Internal error
pub const EPLFMT:   Long = -66;  // Invalid program load format
pub const EGSBF:    Long = -67;  // Memory block growth failure
