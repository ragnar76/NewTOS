//! XBIOS system group (Phase 11): `Random` / `Vsync` / `Kbdvbase`.
//! Mirrors EmuTOS `bios/xbios.c` (`random`/`kbdvbase`) and
//! `bios/screen.c` (`vsync`).
//!
//! Spec: `docs/api/xbios/{11-random,25-vsync,22-kbdvbase}.md`.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU16, AtomicU32, Ordering};

use crate::host;
use crate::trap::TrapArgs;

/// `Random` (0x11) — 24-bit pseudo-random via the EmuTOS LCG. Seeded
/// from the host frame clock on first use.
pub fn random() -> i64 {
    static RSEED: AtomicU32 = AtomicU32::new(0);
    let mut s = RSEED.load(Ordering::Relaxed);
    if s == 0 {
        let fc = host::host().map_or(1, |h| h.frame_clock());
        s = ((fc << 16) as u32).wrapping_add(fc as u32);
        if s == 0 {
            s = 1;
        }
    }
    // EmuTOS: rseed = rseed * 3141592621 + 1; return (rseed >> 8) & 0xFFFFFF.
    s = s.wrapping_mul(3_141_592_621).wrapping_add(1);
    RSEED.store(s, Ordering::Relaxed);
    ((s >> 8) & 0x00FF_FFFF) as i64
}

/// `Vsync` (0x25) — block until the next frame. Spins on the host frame
/// clock (~60 Hz, derived from the hardware timer); ±1 frame accuracy.
pub fn vsync() -> i64 {
    if let Some(h) = host::host() {
        let a = h.frame_clock();
        while h.frame_clock() == a {
            core::hint::spin_loop();
        }
    }
    0
}

/// `Blitmode` (0x40) — get/set the BLiTTER mode, returning the status
/// word. The Pi/BCM2711 has no Atari BLiTTER, so the contract answer is
/// `0x0000` (bit 1 = "present" clear → absent), for both the `-1` query
/// and any set request — there is nothing to enable. This is a real
/// status value, not an error: a blitter-presence probe correctly sees
/// "absent" and falls back to CPU/VideoCore blits.
///
/// Spec: `docs/api/xbios/40-blitmode.md` §"Pi / BCM2711 binding";
/// `docs/HARDWARE-PROFILE.md` §3.9.
pub fn blitmode() -> i64 {
    0x0000
}

/// `Scrdmp` (0x14) — trigger a screen dump through the installed
/// `dump_vec`, then reset `dumpflg`. The Pi has no printer, and EmuTOS's
/// own default `dump_vec` is `just_rts` — a no-op — so a stock `Scrdmp`
/// does nothing observable. That is exactly the spec's "Default no-op"
/// behaviour, so on NewTOS this is a documented no-op returning `void`
/// (→ 0): there is no dump target, and `dumpflg` (the Alt-Help re-arm
/// flag) has no consumer here. Installing a custom `dump_vec` depends on
/// the deferred BIOS vector dispatch (STEP2-VALIDATION Y2).
///
/// Spec: `docs/api/xbios/14-scrdmp.md` ("Default no-op" criterion).
pub fn scrdmp() -> i64 {
    0
}

/// `Midiws` (0x0C) — write `cnt + 1` bytes (unsigned count) to the MIDI
/// port. The Pi has no MIDI ACIA, so — exactly as on an EmuTOS build
/// without `CONF_WITH_MIDI_ACIA` — the bytes have no transport and are
/// dropped; the call transmits nothing and returns without error
/// (`void` → 0). Wired for ABI completeness so a caller sees the
/// contractual no-op, not the generic `EINVFN` catch-all.
///
/// Spec: `docs/api/xbios/0c-midiws.md` ("No-MIDI profile" criterion).
pub fn midiws() -> i64 {
    0
}

/// `Bconmap` (0x2C) — remap the "current RS-232" logical device onto one
/// of several physical serial ports. The Pi has a single serial port
/// (PL011 UART0) and no SCC / TT-MFP, so `BCONMAP_AVAILABLE` is false —
/// as on a single-serial EmuTOS build — and the contract return is the
/// function number `0x2c` ("feature not available"), independent of
/// `dev` (the body is compiled out when the feature is absent). This is
/// a real contractual value (the D-008 "unimplemented" signal), not the
/// generic `EINVFN`: a caller probing for remappable ports learns there
/// is one fixed port.
///
/// Spec: `docs/api/xbios/2c-bconmap.md` (`BCONMAP_AVAILABLE == 0` →
/// `0x2c`); `docs/HARDWARE-PROFILE.md` §3.2.
pub fn bconmap() -> i64 {
    0x2c
}

/// IKBD vector table (Atari `struct kbdvecs`, 9 handler pointers).
/// Programs read it via `Kbdvbase` and may install their own handlers.
/// Native pointer width (not the m68k 4-byte layout); the IKBD-event
/// dispatch through these vectors lands with the process/AES model.
#[repr(C)]
#[derive(Default)]
struct KbdVecs {
    midivec: usize,
    vkbderr: usize,
    vmiderr: usize,
    statvec: usize,
    mousevec: usize,
    clockvec: usize,
    joyvec: usize,
    midisys: usize,
    ikbdsys: usize,
}

struct KbdVecsCell(UnsafeCell<KbdVecs>);
// SAFETY: single-CPU, IRQ-off trap path; programs poke it cooperatively.
unsafe impl Sync for KbdVecsCell {}
static KBDVECS: KbdVecsCell = KbdVecsCell(UnsafeCell::new(KbdVecs {
    midivec: 0,
    vkbderr: 0,
    vmiderr: 0,
    statvec: 0,
    mousevec: 0,
    clockvec: 0,
    joyvec: 0,
    midisys: 0,
    ikbdsys: 0,
}));

/// `Kbdvbase` (0x22) — pointer to the IKBD vector table.
pub fn kbdvbase() -> i64 {
    KBDVECS.0.get() as i64
}

// ── YM2149 PSG group (0x1C/0x1D/0x1E/0x20) ────────────────────────────────
//
// The Pi/BCM2711 has no Yamaha YM2149. As on an EmuTOS build without
// `CONF_WITH_YM2149`, these take the `#else` branch: `Giaccess`/`Dosound`
// return `0`, `Ongibit`/`Offgibit` are no-ops. This is the contract's
// plausible-zero value, **not** the `xbios_unimpl` function-number pattern
// (callers probing `Giaccess(0, n) == 0` must see 0). Spec:
// `docs/api/xbios/_psg-group.md`.

/// `Giaccess` (0x1C) — read/write a PSG register. No PSG → returns `0`.
pub fn giaccess() -> i64 {
    0
}

/// `Offgibit` (0x1D) — AND-mask PSG Port A. No PSG → no-op (`void` → 0).
pub fn offgibit() -> i64 {
    0
}

/// `Ongibit` (0x1E) — OR-set PSG Port A. No PSG → no-op (`void` → 0).
pub fn ongibit() -> i64 {
    0
}

/// `Dosound` (0x20) — run a sound script. No PSG → returns `0`
/// (`Dosound(NULL)` is a consistent no-op). Spec: `_psg-group.md`.
pub fn dosound() -> i64 {
    0
}

/// `Ikbdws` (0x19) — write `cnt + 1` bytes to the IKBD controller.
///
/// The Pi has no IKBD 6301 / ACIA data register: the keyboard and mouse
/// arrive as USB-HID reports, so the raw IKBD command string has no wire to
/// reach and no ACIA to poll. As with `Midiws` (no transport → bytes
/// dropped), this is the contract's documented `void` no-op (→ 0), not the
/// generic function-number catch-all. A full implementation would *parse*
/// the IKBD command bytes and re-apply them to the USB→IKBD translator; that
/// interpreter does not exist yet (`docs/api/xbios/19-ikbdws.md` §Pi
/// binding), but the observable contract of `Ikbdws` itself is `void` — a
/// caller cannot read back anything the dropped bytes would have changed
/// through this call.
pub fn ikbdws() -> i64 {
    0
}

// ── Kbrate (0x23) — keyboard auto-repeat timing ───────────────────────────

/// Power-up auto-repeat defaults (`include/sysconf.h:68-69`):
/// `KB_INITIAL = 15` (~300 ms), `KB_REPEAT = 2` (~40 ms).
const KB_INITIAL: u16 = 15;
const KB_REPEAT: u16 = 2;

/// Packed auto-repeat shadow: high byte = initial delay, low byte = repeat
/// interval (both in ~20 ms units). Default `(15 << 8) | 2 = 0x0F02`.
static KB_RATE: AtomicU16 = AtomicU16::new((KB_INITIAL << 8) | KB_REPEAT);

/// `Kbrate` (0x23) — get and/or set the keyboard auto-repeat timing.
///
/// `x0` = `initial`, `x1` = `repeat`. Returns the packed **previous** value
/// `(old_initial << 8) | old_repeat` (WORD, sign-extended). Each parameter
/// `< 0` is a query that leaves that field unchanged; a value `>= 0` is
/// stored masked to a byte (`& 0xFF`). Matches EmuTOS `bios/ikbd.c:439`.
///
/// The two byte fields are a pure software shadow — there is no IKBD 6301 on
/// the Pi. This function's own contract (get/set the two settings + the
/// packed old return) is complete and byte-identical to EmuTOS's `kbrate`,
/// which likewise only reads/writes the two globals; the auto-repeat
/// *effect* is produced by a separate repeat handler
/// (`docs/api/xbios/23-kbrate.md` §Pi binding), independent of this call.
pub fn kbrate(args: &TrapArgs) -> i64 {
    let initial = args.arg_i16(0);
    let repeat = args.arg_i16(1);
    let old = KB_RATE.load(Ordering::Relaxed);
    let mut new = old;
    if initial >= 0 {
        new = (new & 0x00FF) | (((initial as u16) & 0xFF) << 8);
    }
    if repeat >= 0 {
        new = (new & 0xFF00) | ((repeat as u16) & 0xFF);
    }
    if new != old {
        KB_RATE.store(new, Ordering::Relaxed);
    }
    // WORD return, sign-extended into x0.
    old as i16 as i64
}

// ── Setprt (0x21) — desktop printer configuration word ────────────────────

/// Desktop printer configuration word (`bios/parport.c:36,118`). Pure
/// software; default 0 (output via parallel port).
static PRINTER_CONFIG: AtomicU16 = AtomicU16::new(0);

/// `Setprt` (0x21) — set/get the desktop printer configuration word.
///
/// `x0` = `config`. Returns the **previous** config word (WORD,
/// sign-extended); `config == -1` is the query (leaves the word unchanged),
/// any other value is stored. Matches EmuTOS `bios/parport.c:49-57`.
///
/// This is a **pure software** config-word getter/setter — only the separate
/// `prnout` output path touches the actual printer port, so the config word
/// itself is independent of any printer hardware. Honouring the store+return
/// contract (rather than the function-number catch-all) means an
/// `old = Setprt(cfg); …; Setprt(old)` save/restore round-trips correctly,
/// exactly the same shadow convention as `Rsconf`/`Kbrate`/`Cursconf`.
pub fn setprt(args: &TrapArgs) -> i64 {
    let config = args.arg_i16(0);
    let old = PRINTER_CONFIG.load(Ordering::Relaxed);
    if config != -1 {
        PRINTER_CONFIG.store(config as u16, Ordering::Relaxed);
    }
    // WORD return, sign-extended into x0.
    old as i16 as i64
}

// ── Rsconf (0x0F) — RS-232 configuration ──────────────────────────────────
//
// Thin adapter over the PL011 UART0 (no MFP 68901 / ACIA on the Pi). The
// packed old-register return has no hardware source and is synthesised from
// software shadows (`docs/api/xbios/0f-rsconf.md` §Pi binding). The shadow
// defaults are EmuTOS's power-up values (`bios/serport.c:89,1039`): baud
// B9600 = index 1, UCR 0x88, RSR 1, TSR 1, flow NONE.

static RS_BAUD: AtomicU16 = AtomicU16::new(1); // B9600 (DEFAULT_BAUDRATE)
static RS_FLOW: AtomicU16 = AtomicU16::new(0); // FLOW_CTRL_NONE
static RS_UCR: AtomicU16 = AtomicU16::new(0x88);
static RS_RSR: AtomicU16 = AtomicU16::new(0x01);
static RS_TSR: AtomicU16 = AtomicU16::new(0x01);

/// `Rsconf` (0x0F) — configure the RS-232 port.
///
/// `x0` = `speed`, `x1` = `ctrl`, `x2` = `ucr`, `x3` = `rsr`, `x4` = `tsr`,
/// `x5` = `scr`. Contract (EmuTOS `rsconf_mfp`, `bios/serport.c:217-249`,
/// `docs/api/xbios/0f-rsconf.md`):
/// - `speed == -2` → **query**: return the current baud code, change nothing.
/// - `speed` in `0..=15` sets the baud index; `-1` keeps it.
/// - `ctrl` in `0..=3` sets the flow-control mode; `-1` keeps it.
/// - `ucr`/`rsr`/`tsr` each applied only when `>= 0` (stored masked to a
///   byte); `scr` is accepted but neither stored nor returned (EmuTOS omits
///   it, `:229`).
/// - otherwise return the **old** packed `(ucr << 24) | (rsr << 16) |
///   (tsr << 8)` (low byte 0), zero-extended into `x0`.
pub fn rsconf(args: &TrapArgs) -> i64 {
    let speed = args.arg_i16(0);
    let ctrl = args.arg_i16(1);
    let ucr = args.arg_i16(2);
    let rsr = args.arg_i16(3);
    let tsr = args.arg_i16(4);
    // arg5 = scr: ignored (excluded from the return, `serport.c:229`).

    if speed == -2 {
        return RS_BAUD.load(Ordering::Relaxed) as i64;
    }

    let old = ((RS_UCR.load(Ordering::Relaxed) as u32) << 24)
        | ((RS_RSR.load(Ordering::Relaxed) as u32) << 16)
        | ((RS_TSR.load(Ordering::Relaxed) as u32) << 8);

    if (0..=15).contains(&speed) {
        RS_BAUD.store(speed as u16, Ordering::Relaxed);
    }
    if (0..=3).contains(&ctrl) {
        RS_FLOW.store(ctrl as u16, Ordering::Relaxed);
    }
    if ucr >= 0 {
        RS_UCR.store((ucr as u16) & 0xFF, Ordering::Relaxed);
    }
    if rsr >= 0 {
        RS_RSR.store((rsr as u16) & 0xFF, Ordering::Relaxed);
    }
    if tsr >= 0 {
        RS_TSR.store((tsr as u16) & 0xFF, Ordering::Relaxed);
    }
    // ULONG return, zero-extended into x0.
    old as i64
}
