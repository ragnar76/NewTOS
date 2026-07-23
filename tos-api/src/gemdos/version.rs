//! GEMDOS version query (PHASE-07 D5).
//!
//! Spec: `docs/api/gemdos/version/30-sversion.md`.

use crate::trap::TrapArgs;
use tos_types::Long;

/// GEMDOS version NewTOS reports. NewTOS targets the Falcon030 / TOS 4.04
/// profile, so it reports the value real Falcon TOS 4.0x returns: `0x3000`
/// (= "GEMDOS 0.30"). EmuTOS' own default is the more conservative `0x2000`,
/// but its `config.h` documents `0x3000` as the value "used by TOS v4.0x" —
/// which is our reference machine. The Atari encoding is byte-swapped: low
/// byte = major, high byte = minor (`0x3000` ⇒ major 0x00, minor 0x30).
const GEMDOS_VERSION: Long = 0x0000_3000;

/// `Sversion()` (0x30) — return the GEMDOS version word.
pub fn sversion(_: &TrapArgs) -> Long {
    GEMDOS_VERSION
}
