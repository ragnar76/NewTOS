//! EdDI `vq_scrninfo` — VDI opcode 102, sub-opcode 1, `INTIN[0] == 2`.
//!
//! Spec: [`docs/api/vdi/eddi-vq_scrninfo.md`] (approved 2026-10-02, E1–E9 as
//! recommended in its §7, **E3 = `0x02`**). EmuTOS has **no** EdDI (spec §0):
//! this is a deliberate NewTOS extension beyond EmuTOS, recorded in
//! `docs/COMPATIBILITY.md` (E8). The `EdDI` cookie that announces it is
//! seeded in [`crate::cookie::seed_defaults`] ([`crate::cookie::EDDI`]).
//!
//! **What is answered.** The NewTOS device word is the Falcon HiColor word
//! `RRRRRGGG GG0BBBBB` — 5/5/5 bits of red/green/blue, bit 5 the `OVERLAY_BIT`
//! the VDI never sets (spec §6.2; EmuTOS `vdi_col.c:599-613`, `vdi_fill.c:26`;
//! [`super::vwk::pack_color16`]). That is bit for bit the Falcon HiColor
//! example of the EdDI documentation (spec §3.3/§6.3), so NewTOS answers what
//! a Falcon answers: packed pixels, software CLUT, 16 planes, 32768 colours,
//! the bit-number tables of §6.3 "Fall B" and bit order `0x02`.
//!
//! The bit numbers describe the pixel **word as the CPU reads it** (E3): a
//! NewTOS program and `vro_cpyfm` read the same native `u16` out of a
//! device-format memory MFDB (`raster.rs`, `Mfdb::get_mem`/`put_mem`), so there is
//! no byte-order difference to report and bit 7 (`0x80`, "swapped bytes")
//! stays clear.
//!
//! Measured counterpart: the VDI Enhancer 1.00 (a real EdDI) on EmuTOS
//! Falcon truecolor reports the same 273 words except `[14]`
//! (`measurements/measurement-2026-10-02-scrninfo/hatari-falcon-tc-enhancer/
//! SCRNINFO.TXT`). Every word that differs is named at [`scrninfo_table`].

use super::backend::RasterBackend;
use super::Vdipb;

/// `CONTRL[5]` sub-opcode that turns opcode 102 into `vq_scrninfo`
/// (spec §3.1; gemlib `n_vq_scrninfo.c`, `VDI_TRAP_ESC(...,102,1,0,1)`).
pub(super) const SCRNINFO_SUBOP: i16 = 1;

/// `INTIN[0]` value of `vq_scrninfo` — "output extended information"
/// (spec §3.1, NV250 §2).
pub(super) const SCRNINFO_MODE: i16 = 2;

/// Number of `work_out` words written: `INTOUT[0..=272]`. The documentation
/// gives `CONTRL[4] = 272` but describes 273 words, and the NVDI 2.5x
/// Enhancer copies 273 (spec §3.1); the measured Enhancer 1.00 writes
/// `INTOUT[272]` (= 0) and nothing beyond.
pub const SCRNINFO_WORDS: usize = 273;

/// `CONTRL[4]` as the documentation states it (spec §3.1: "`CONTRL[4]` |
/// 272 (Doku)" — "der Treiber traegt ihn ein").
pub const SCRNINFO_NINTOUT: i16 = 272;

/// The only device word NewTOS has: 16 planes (`kernel/src/vdi_backend.rs`
/// `PLANES`, ADR-024). The spec derives an answer for exactly this format
/// (§6.2/§6.3); see [`is_scrninfo`] for any other plane count.
const DEVICE_PLANES: u16 = 16;

// The device word's layout, as `pack_color16` packs it (`(r << 11) |
// (g << 6) | b`, `vwk.rs`) and `OVERLAY_BIT` (`0x0020`, EmuTOS
// `vdi_fill.c:26`) — the inputs of the bit-number tables.
const GUN_BITS: i16 = 5;
const RED_LSB: i16 = 11;
const GREEN_LSB: i16 = 6;
const BLUE_LSB: i16 = 0;
const GENLOCK_BIT: i16 = 5;

/// `work_out[14]` bit order: `2` = Falcon order `rrrrrggg-ggxbbbbb`, one
/// overlay bit in the middle (spec §3.3; gemlib `n_vq_scrninfo.c:144`).
/// Approved as E3 (spec status header).
const ORDER_FALCON: i16 = 0x02;

/// Is this opcode-102 call a `vq_scrninfo`?
///
/// E7: `CONTRL[5] == 1` **and** `INTIN[0] == 2`; every other combination
/// stays `vq_extnd` exactly as before (spec §7 E7). This is the one place
/// NewTOS reads `CONTRL[5]` for opcode 102, which EmuTOS does not
/// (`vdi_main.c:77`) — the deviation E7/E8 names.
///
/// The answer is defined for the 16-plane device word only (spec §6.3). A
/// backend with any other plane count — none exists in NewTOS, whose screen
/// is fixed at 16 planes (ADR-024); only host test doubles differ — keeps the
/// pre-EdDI `vq_extnd` answer rather than a table the spec never derived.
pub(super) fn is_scrninfo(pb: &Vdipb, backend: &dyn RasterBackend) -> bool {
    pb.contrl(5) == SCRNINFO_SUBOP
        && pb.intin(0) == SCRNINFO_MODE
        && backend.planes() == DEVICE_PLANES
}

/// The 273 `work_out` words NewTOS reports (spec §3.2 layout, §6.3 values).
///
/// Word by word against the measured Enhancer 1.00 on EmuTOS Falcon
/// truecolor (`.../hatari-falcon-tc-enhancer/SCRNINFO.TXT`, §3b):
///
/// | idx | NewTOS | Enhancer | why |
/// |---|---|---|---|
/// | 0 | 2 | 2 | packed pixels |
/// | 1 | 2 | 2 | software CLUT (`INQ_TAB[5] = 0`) |
/// | 2 | 16 | 16 | `v_planes` |
/// | 3/4 | 0 / `0x8000` | 0 / `0x8000` | 32768 colours, high word first (E4) |
/// | 5 | 0 | 0 | no accessible line width (E5) |
/// | 6/7 | 0 / 0 | 0 / 0 | no accessible bitmap address (E5) |
/// | 8/9/10 | 5/5/5 | 5/5/5 | bits per gun |
/// | 11 | 0 | 0 | no alpha |
/// | 12 | 1 | 1 | one genlock bit (`OVERLAY_BIT`); programs leave it 0 (E9) |
/// | 13 | 0 | 0 | no unused bits |
/// | **14** | **2** | **0** | **differs**: the Enhancer is EdDI 1.00, which has no `[14]` (it arrives with 1.10, spec §2) and leaves it 0; NewTOS fills it with the Falcon order (E3) |
/// | 15 | 0 | 0 | undocumented |
/// | 16..20 | 11..15 | 11..15 | red bit numbers, LSB first |
/// | 32..36 | 6..10 | 6..10 | green |
/// | 48..52 | 0..4 | 0..4 | blue |
/// | 80 | 5 | 5 | genlock bit |
/// | rest of 16..127 | -1 | -1 | unused table entries |
/// | 128..272 | 0 | 0 | reserved |
pub fn scrninfo_table() -> [i16; SCRNINFO_WORDS] {
    let mut w = [0i16; SCRNINFO_WORDS];
    w[0] = 2; // format: packed pixels
    w[1] = 2; // CLUT: software ("pseudo") CLUT, HiColor/TrueColor
    w[2] = DEVICE_PLANES as i16; // bits per pixel
    // Colour count as a LONG, high word first (spec §3.2 [3/4]): 32768. Not
    // 65536 — the overlay bit carries no colour (E4).
    let colours: u32 = 1 << (3 * GUN_BITS);
    w[3] = (colours >> 16) as i16;
    w[4] = colours as u16 as i16;
    // [5] line width, [6/7] bitmap address: 0 (E5).
    w[8] = GUN_BITS; // red
    w[9] = GUN_BITS; // green
    w[10] = GUN_BITS; // blue
    w[11] = 0; // alpha
    w[12] = 1; // genlock
    w[13] = 0; // unused
    w[14] = ORDER_FALCON;
    // [15]: undocumented, 0.

    // Bit-number tables 16..127: -1 = "not used", then the bits present.
    for v in &mut w[16..128] {
        *v = -1;
    }
    for i in 0..GUN_BITS {
        w[16 + i as usize] = RED_LSB + i;
        w[32 + i as usize] = GREEN_LSB + i;
        w[48 + i as usize] = BLUE_LSB + i;
    }
    w[80] = GENLOCK_BIT;
    // [128..=272]: reserved, 0.
    w
}

/// Opcode 102 sub-opcode 1, `INTIN[0] == 2` — `vq_scrninfo`.
///
/// Writes `INTOUT[0..=272]` ([`scrninfo_table`]); PTSOUT is not touched
/// (spec §3.1 — `ptsout` is not to be written by the driver; the measured
/// Enhancer writes none). Every valid screen handle, physical or virtual,
/// gets the same answer (E6); an invalid one never gets here, the dispatcher
/// has already turned it into a no-op as for `vq_extnd`.
///
/// Output counts: `CONTRL[4] = 272` as documented (spec §3.1), `CONTRL[2] =
/// 0` (no PTSOUT). The Enhancer leaves both as the caller left them; NewTOS'
/// dispatcher rewrites the counts on every call (as EmuTOS' `screen()`
/// does), so it states the documented values instead of the zero pre-load.
pub(super) fn vq_scrninfo(pb: &Vdipb) {
    for (i, &v) in scrninfo_table().iter().enumerate() {
        pb.set_intout(i, v);
    }
    pb.set_contrl(4, SCRNINFO_NINTOUT);
    pb.set_contrl(2, 0);
}
