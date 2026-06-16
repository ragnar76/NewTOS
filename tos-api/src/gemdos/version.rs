//! GEMDOS version query (PHASE-07 D5).
//!
//! Spec: `docs/api/gemdos/version/30-sversion.md`.

use crate::trap::TrapArgs;
use tos_types::Long;

/// GEMDOS version NewTOS reports. EmuTOS's default is `0x2000`
/// (= "GEMDOS 0.32"); the Atari encoding is byte-swapped — low byte =
/// major, high byte = minor (`0x2000` ⇒ major 0x00, minor 0x20).
/// NewTOS reports the same value so ported programs that gate on a
/// GEMDOS version see a standard, modern-enough GEMDOS.
const GEMDOS_VERSION: Long = 0x0000_2000;

/// `Sversion()` (0x30) — return the GEMDOS version word.
pub fn sversion(_: &TrapArgs) -> Long {
    GEMDOS_VERSION
}
