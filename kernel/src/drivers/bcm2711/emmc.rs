//! BCM2711 EMMC2 (Arasan/iproc SDHCI) microSD driver.
//!
//! Reference: [`docs/hardware/emmc-sd.md`] — the on-board microSD card
//! slot (EMMC2 @ `0xFE34_0000`), the SD Host Controller + SD Physical
//! Layer specs. Bring-up scope: PIO single-block **read** (no DMA, no
//! IRQ). First test vehicle is QEMU `raspi4b`'s `emmc` device.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::hal::block::{BlockDevice, BlockError, SECTOR_SIZE};

/// SD-host controller candidate bases (low-peripheral mode). On real
/// RPi4 the microSD slot is **EMMC2** (`0xFE34_0000`); QEMU `raspi4b`
/// wires its emulated card to the legacy **EMMC** SDHCI (`0xFE30_0000`).
/// Both are standard SDHCI, so one driver serves both — `init()` probes
/// EMMC2 first (real HW), then EMMC (QEMU).
const EMMC2_BASE: usize = crate::drivers::mmio_va(0xFE34_0000);
const EMMC_BASE: usize = crate::drivers::mmio_va(0xFE30_0000);

/// The controller base the driver is currently bound to.
static BASE_ADDR: AtomicUsize = AtomicUsize::new(EMMC2_BASE);

fn base() -> usize {
    BASE_ADDR.load(Ordering::Relaxed)
}

// Register offsets (datasheet §5 grouping; see the Track-B reference).
const BLKSIZECNT: usize = 0x04;
const ARG1: usize = 0x08;
const CMDTM: usize = 0x0C;
const RESP0: usize = 0x10;
const RESP1: usize = 0x14;
const RESP2: usize = 0x18;
const RESP3: usize = 0x1C;
const DATA: usize = 0x20;
const STATUS: usize = 0x24;
const CONTROL0: usize = 0x28;
const CONTROL1: usize = 0x2C;
const INTERRUPT: usize = 0x30;
const IRPT_MASK: usize = 0x34;
const IRPT_EN: usize = 0x38;
const CAPABILITIES: usize = 0x40;
const SLOTISR_VER: usize = 0xFC;

// STATUS (present state) bits.
const SR_CMD_INHIBIT: u32 = 1 << 0;
const SR_DAT_INHIBIT: u32 = 1 << 1;

// CONTROL1 bits.
const C1_CLK_INTLEN: u32 = 1 << 0;
const C1_CLK_STABLE: u32 = 1 << 1;
const C1_CLK_EN: u32 = 1 << 2;
const C1_TOUNIT_MAX: u32 = 0xE << 16;
const C1_SRST_HC: u32 = 1 << 24;
/// `Software Reset For CMD Line` — resets the controller's command circuit.
/// SD Host Controller Simplified Spec 3.00 §2.2.15; same bit in the BCM
/// Arasan block (`docs/hardware/emmc-sd.md` §1 "CONTROL1", bit 25) and in
/// the pinned RPi4 reference (`references/circle/addon/SDCard/emmc.cpp:523`).
const C1_SRST_CMD: u32 = 1 << 25;
/// `Software Reset For DAT Line` — resets the controller's data circuit.
/// Same sources as [`C1_SRST_CMD`] (bit 26, `emmc.cpp:524`).
const C1_SRST_DATA: u32 = 1 << 26;

// INTERRUPT bits.
const INT_CMD_DONE: u32 = 1 << 0;
const INT_DATA_DONE: u32 = 1 << 1;
const INT_WRITE_RDY: u32 = 1 << 4;
const INT_READ_RDY: u32 = 1 << 5;
const INT_ERROR_MASK: u32 = 0xFFFF_0000;

// INTERRUPT error bits (upper half), SD-HC 3.00 §2.2.18 "Error Interrupt
// Status". Bit positions cross-checked against the pinned RPi4 reference
// (`references/circle/addon/SDCard/emmc.cpp:153-165`) and against the set
// the FS04 audit asks the driver to report instead of silently clearing
// (`docs/api/hw/emmc-audit.md` §5 M2: 16 CTO, 17 CCRC, 20 DTO, 21 DCRC,
// 22 DEND).
const INT_ERR_CTO: u32 = 1 << 16; // Command Timeout Error
const INT_ERR_CCRC: u32 = 1 << 17; // Command CRC Error
const INT_ERR_CEND: u32 = 1 << 18; // Command End Bit Error
const INT_ERR_CBAD: u32 = 1 << 19; // Command Index Error
const INT_ERR_DTO: u32 = 1 << 20; // Data Timeout Error
const INT_ERR_DCRC: u32 = 1 << 21; // Data CRC Error
const INT_ERR_DEND: u32 = 1 << 22; // Data End Bit Error

// ── R1 card status (SD Phys 9.10 Table 4-42, `docs/hardware/emmc-sd.md`
// §5) ────────────────────────────────────────────────────────────────
//
// An R1 response carries the card's own verdict on the command. The
// controller's `INTERRUPT` register says nothing about it: a command the
// card *refuses* still completes on the bus without a single error bit
// (audit finding 3 drove exactly that — a write past the end of the card
// returned `Ok(())` with `INTERRUPT = 0`).
//
// Listed below are the entries whose *Type* column in Table 4-42 contains
// `E` (error bit); status entries (`CURRENT_STATE`, `READY_FOR_DATA`,
// `APP_CMD`, `CARD_IS_LOCKED`, `CARD_ECC_DISABLED`, `ERASE_RESET`,
// `FX_EVENT`) are deliberately **not** here — they are not failures.
//
// Each comment below says in our own words what the bit means *for this
// driver*, not what the specification says about it; the long form, with
// the per-command cross-reference and the error-code mapping, is in
// `docs/hardware/emmc-sd.md` §5. All fifteen share one baseline:
// `check_r1()` tests them as a single mask, so any of them turns the call
// into `Err(BlockError::Card)` → `EREADF` / `EWRITF`; the per-bit counter
// in `ErrorStats` is diagnostic only.

/// Bit 31 — the block address we sent lies outside what the card accepts.
/// One of the two bits a `CMD17`/`CMD24` response realistically carries,
/// and the reason `check_r1()` exists at all: before it, a write past the
/// end of the card returned `Ok(())`. SD Phys 9.10 §4.10.1.
const R1_OUT_OF_RANGE: u32 = 1 << 31;
/// Bit 30 — the card rejected our address against its block length. The
/// other bit Table 4-43 lists for `CMD17`/`CMD24`; `card_address()` cannot
/// produce a misaligned argument, so this fires only if the card disagrees
/// with our own arithmetic.
const R1_ADDRESS_ERROR: u32 = 1 << 30;
/// Bit 29 — the card rejected the transfer length. We always request
/// exactly `SECTOR_SIZE` and pin an SDSC card to 512 bytes during init, so
/// nothing in our own request should reach this.
const R1_BLOCK_LEN_ERROR: u32 = 1 << 29;
/// Bit 28 — the card's erase sequence went wrong. We issue no erase
/// command, so this cannot arise from our own traffic; it stays in the
/// mask so a card that sets it anyway is still treated as a failure.
const R1_ERASE_SEQ_ERROR: u32 = 1 << 28;
/// Bit 27 — the card rejected an erase block range. Unreachable for the
/// same reason as `R1_ERASE_SEQ_ERROR`: we never erase.
const R1_ERASE_PARAM: u32 = 1 << 27;
/// Bit 26 — a write hit protected storage. Reachable through `CMD24`, our
/// only write command; like EmuTOS' own SD driver we fold it into the
/// generic `EWRITF` instead of a write-protect-specific code.
const R1_WP_VIOLATION: u32 = 1 << 26;
/// Bit 24 — the card's lock/unlock feature refused the request. We never
/// issue that command, so this is unreachable in practice.
const R1_LOCK_UNLOCK_FAILED: u32 = 1 << 24;
/// Bit 23 — the *card* found the command it received corrupted. Distinct
/// from `INT_ERR_CCRC`, which is the *controller's* verdict on the same
/// kind of failure and is handled earlier, in `wait_interrupt()`.
const R1_COM_CRC_ERROR: u32 = 1 << 23;
/// Bit 22 — the card refuses the command in the state it is currently in.
/// Reachable on either of our two data commands; no special handling.
const R1_ILLEGAL_COMMAND: u32 = 1 << 22;
/// Bit 21 — the card's internal ECC could not repair the data it read.
/// Reachable on `CMD17`; we neither retry nor report it separately.
const R1_CARD_ECC_FAILED: u32 = 1 << 21;
/// Bit 20 — the card's own controller failed internally. Generic and
/// reachable on either command, with no special handling.
const R1_CC_ERROR: u32 = 1 << 20;
/// Bit 19 — the card's catch-all rejection, given when it has no finer
/// reason to report. Beyond its own counter it is indistinguishable here
/// from any other bit in this list.
const R1_ERROR: u32 = 1 << 19;
/// Bit 16 — an illegal attempt to change a read-only CSD field or undo a
/// permanent write-protect bit. We issue none of the programming commands
/// this covers, so it is unreachable via `CMD17`/`CMD24`.
const R1_CSD_OVERWRITE: u32 = 1 << 16;
/// Bit 15 — an erase left write-protected blocks untouched. Unreachable
/// like the other two erase-family bits: we never erase.
const R1_WP_ERASE_SKIP: u32 = 1 << 15;
/// Bit 3 — the card's authentication sequence went wrong. We run no
/// authentication, so this is unreachable.
const R1_AKE_SEQ_ERROR: u32 = 1 << 3;

/// Every Type-`E` bit of Table 4-42 — a non-zero AND means the card
/// refused or failed the command.
const R1_ERROR_MASK: u32 = R1_OUT_OF_RANGE
    | R1_ADDRESS_ERROR
    | R1_BLOCK_LEN_ERROR
    | R1_ERASE_SEQ_ERROR
    | R1_ERASE_PARAM
    | R1_WP_VIOLATION
    | R1_LOCK_UNLOCK_FAILED
    | R1_COM_CRC_ERROR
    | R1_ILLEGAL_COMMAND
    | R1_CARD_ECC_FAILED
    | R1_CC_ERROR
    | R1_ERROR
    | R1_CSD_OVERWRITE
    | R1_WP_ERASE_SKIP
    | R1_AKE_SEQ_ERROR;

/// The R1 error bits the driver counts by name, in [`R1_BIT_NAMES`] order.
const R1_BIT_MASKS: [u32; 15] = [
    R1_OUT_OF_RANGE,
    R1_ADDRESS_ERROR,
    R1_BLOCK_LEN_ERROR,
    R1_ERASE_SEQ_ERROR,
    R1_ERASE_PARAM,
    R1_WP_VIOLATION,
    R1_LOCK_UNLOCK_FAILED,
    R1_COM_CRC_ERROR,
    R1_ILLEGAL_COMMAND,
    R1_CARD_ECC_FAILED,
    R1_CC_ERROR,
    R1_ERROR,
    R1_CSD_OVERWRITE,
    R1_WP_ERASE_SKIP,
    R1_AKE_SEQ_ERROR,
];

/// Names of the counted R1 error bits, in [`R1_BIT_MASKS`] order.
pub const R1_BIT_NAMES: [&str; 15] = [
    "OUT_OF_RANGE",
    "ADDRESS_ERROR",
    "BLOCK_LEN_ERROR",
    "ERASE_SEQ_ERROR",
    "ERASE_PARAM",
    "WP_VIOLATION",
    "LOCK_UNLOCK_FAILED",
    "COM_CRC_ERROR",
    "ILLEGAL_COMMAND",
    "CARD_ECC_FAILED",
    "CC_ERROR",
    "ERROR",
    "CSD_OVERWRITE",
    "WP_ERASE_SKIP",
    "AKE_SEQ_ERROR",
];

/// The error bits that belong to the **command** line — SD-HC §3.10.1 asks
/// for `Software Reset For CMD Line` when one of these is set.
const INT_ERR_CMD_MASK: u32 = INT_ERR_CTO | INT_ERR_CCRC | INT_ERR_CEND | INT_ERR_CBAD;
/// The error bits that belong to the **data** line — SD-HC §3.10.1 asks for
/// `Software Reset For DAT Line` when one of these is set.
const INT_ERR_DAT_MASK: u32 = INT_ERR_DTO | INT_ERR_DCRC | INT_ERR_DEND;

// CMDTM command-word flags (upper half).
const RSP_NONE: u32 = 0;
const RSP_136: u32 = 1 << 16;
const RSP_48: u32 = 2 << 16;
const RSP_48BUSY: u32 = 3 << 16;
const CMD_CRCCHK: u32 = 1 << 19;
const CMD_IXCHK: u32 = 1 << 20;
const CMD_ISDATA: u32 = 1 << 21;
/// `CMDTM` Command Type = **abort** (bits 23:22 = 11b). An abort command is
/// the one command the host may issue while Command Inhibit (DAT) is still
/// set — the reference driver skips its DAT-line wait for exactly this
/// command type (`references/circle/addon/SDCard/emmc.cpp:133` and
/// `:988-991`), which is what makes CMD12 usable as the abort of a stuck
/// data transaction (SD-HC 3.00 §3.8).
const CMD_TYPE_ABORT: u32 = 3 << 22;
const fn cmd(index: u32) -> u32 {
    index << 24
}
// Transfer-mode flags (lower half).
const TM_DAT_CARD2HOST: u32 = 1 << 4;

// SD commands as full CMDTM words.
const GO_IDLE: u32 = cmd(0) | RSP_NONE;
const SEND_IF_COND: u32 = cmd(8) | RSP_48 | CMD_CRCCHK | CMD_IXCHK;
const ALL_SEND_CID: u32 = cmd(2) | RSP_136 | CMD_CRCCHK;
const SEND_REL_ADDR: u32 = cmd(3) | RSP_48 | CMD_CRCCHK | CMD_IXCHK;
const SEND_CSD: u32 = cmd(9) | RSP_136 | CMD_CRCCHK;
const SELECT_CARD: u32 = cmd(7) | RSP_48BUSY | CMD_CRCCHK | CMD_IXCHK;
const SET_BLOCKLEN: u32 = cmd(16) | RSP_48 | CMD_CRCCHK | CMD_IXCHK;
const READ_SINGLE: u32 =
    cmd(17) | RSP_48 | CMD_CRCCHK | CMD_IXCHK | CMD_ISDATA | TM_DAT_CARD2HOST;
const WRITE_SINGLE: u32 = cmd(24) | RSP_48 | CMD_CRCCHK | CMD_IXCHK | CMD_ISDATA;
const APP_CMD: u32 = cmd(55) | RSP_48 | CMD_CRCCHK | CMD_IXCHK;
const SD_SEND_OP_COND: u32 = cmd(41) | RSP_48; // ACMD41, R3 (no CRC/index check)
/// CMD12 `STOP_TRANSMISSION`, issued as the **abort command** of SD-HC §3.8.
/// R1b with CRC check, no index check — the flag set the pinned reference
/// uses for this command (`emmc.cpp:284`, `SD_CMD_INDEX(12) | SD_RESP_R1b |
/// SD_CMD_TYPE_ABORT`, with `SD_RESP_R1b = RSPNS_TYPE_48B | CRCCHK_EN`
/// at `:193`).
const STOP_TRANSMISSION: u32 = cmd(12) | RSP_48BUSY | CMD_CRCCHK | CMD_TYPE_ABORT;

/// Busy-wait iteration budget for the **initialisation-phase** register
/// polls only: the controller reset, the clock-stable wait and the ACMD41
/// power-up loop. Those are audit findings 9 and 11 and belong to their own
/// tickets; the per-sector wait phases no longer use an iteration count at
/// all — they use [`Budget`], a real duration.
const SPIN_LIMIT: u32 = 1_000_000;

// ─────────────────────────────────────────────────────────────────────
// Wait budgets — a timeout is a duration, not an iteration count
// ─────────────────────────────────────────────────────────────────────
//
// FS04 audit finding 1 (`docs/api/hw/emmc-audit.md` §3.1, §4): the five
// waits of a sector operation used to share one iteration counter. On one
// and the same Raspberry Pi 4, inside a single measurement run, an
// iteration cost 104 ns in one block and 201 ns in another (the CPU clock
// moved) — so the "timeout" was 104–201 ms depending on how fast the
// machine happened to be running, while the specification allows the card
// 250 ms of programming time per write and 500 ms for the last busy.
// Twelve of 23 sector writes ran into that limit and were silently
// retried (audit §3.2).
//
// Every limit below is therefore a duration measured on the ARM generic
// timer, and each wait phase gets the limit the specification gives *that*
// phase. The SD specifications are not in `references/` (audit finding
// S1), so each quote is cited through the audit, which carries it verbatim.

/// Longest busy a write may legally take, in milliseconds.
///
/// SD Physical Layer Simplified Spec 6.00 §4.6.2.2: *"In case of High
/// Capacity SD Memory Card, maximum length of busy is defined as **250 ms**
/// for all write operation"*, and *"a) The last busy in any write operation
/// up to **500 ms** including single and multiple block write"*; its
/// Application Note: *"It is **strongly recommended for hosts to implement
/// more than 500 ms timeout value** even if the card indicates the 250 ms
/// maximum busy length"* (quoted in `docs/api/hw/emmc-audit.md` §3.1).
///
/// 1000 ms is that "more than 500 ms", with a factor-2 margin over the
/// longest busy the specification permits. It is also shorter than the
/// controller's own data timeout (`DATA_TOUNIT` = 0xE, measured at 2.58 s
/// in QEMU, audit §5 M2), so a card that exceeds its own specification is
/// caught here and recovered from rather than stalling the machine.
const WRITE_BUSY_MS: u32 = 1_000;

/// Longest read the host must tolerate, in milliseconds.
///
/// SD Phys 6.00 §4.6.2.1: *"The host should use **100 ms timeout
/// (minimum)** for single and multiple read operations"* (audit §3.1). That
/// is a *minimum*, and the audit counts it against the old budget that it
/// "sits exactly on the lower bound" (§3.1) — so this takes twice it, the
/// same factor-2 margin as [`WRITE_BUSY_MS`]. Measured read waits on the Pi
/// were 0.5–1.1 ms (audit §3.3), three orders of magnitude below.
const READ_MS: u32 = 200;

/// Backstop for the phases the SD Physical spec puts no *card*-side limit
/// on: the two Command Inhibit waits, Command Complete and Buffer Write
/// Ready. The card's response time is bounded by N_CR ≤ 64 clock cycles and
/// the controller enforces it in hardware (`Command Timeout Error`, SD-HC
/// §2.2.18), so a software limit here only bounds a *wedged controller*. It
/// is set to [`WRITE_BUSY_MS`], the longest card-side wait the specification
/// allows anywhere, so that no single wait can stall the system longer than
/// that.
const CTRL_MS: u32 = WRITE_BUSY_MS;

/// Budget for the recovery steps themselves (software reset, abort
/// command). Neither has a specified duration — this only keeps error
/// recovery from becoming the next hang; the pinned reference waits 1 s for
/// the same reset bits (`emmc.cpp:945`, `:960`). When it runs out, the
/// controller is reported as SD-HC §3.10.1 "Non-recoverable Error".
const RECOVERY_MS: u32 = 100;

/// Read `CNTPCT_EL0` — the free-running physical counter of the ARM generic
/// timer, the same clock source `drivers::timer` uses for the 200 Hz tick.
#[inline]
fn cntpct() -> u64 {
    let v: u64;
    // SAFETY: read-only system-register read, permitted at EL1.
    unsafe {
        core::arch::asm!("mrs {0}, cntpct_el0", out(reg) v,
                         options(nomem, nostack, preserves_flags));
    }
    v
}

/// Read `CNTFRQ_EL0` — the counter frequency in Hz, as the machine reports
/// it (0 when it reports none; see [`Budget::Spins`]).
#[inline]
fn cntfrq() -> u64 {
    let v: u64;
    // SAFETY: read-only system-register read, permitted at EL1.
    unsafe {
        core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) v,
                         options(nomem, nostack, preserves_flags));
    }
    v
}

/// Counter ticks for `ms` milliseconds at `frq` Hz — never zero, so a wait
/// always gets at least one tick of budget.
const fn ticks_for_ms(frq: u64, ms: u32) -> u64 {
    let t = frq.saturating_mul(ms as u64) / 1000;
    if t == 0 { 1 } else { t }
}

/// Has a wait that started at counter value `start` reached `limit` ticks?
///
/// Only the *difference* of the two counter readings is used, so the test
/// survives the counter wrapping: `now.wrapping_sub(start)` is the true
/// elapsed count as long as the wait is shorter than a full counter period.
/// If the implemented counter is narrower than the 64-bit register and rolls
/// over to zero mid-wait, the difference jumps to a huge value and the wait
/// ends early — a timeout that error recovery handles, not a hang, and at
/// the counter frequencies these boards report (54 MHz on the Pi 4) that is
/// decades away.
const fn reached(start: u64, now: u64, limit: u64) -> bool {
    now.wrapping_sub(start) >= limit
}

/// The wait budget of one polling phase.
enum Budget {
    /// Normal case: the counter value the wait started at, and the number of
    /// counter ticks it may take.
    Ticks { start: u64, limit: u64 },
    /// `CNTFRQ_EL0` reads 0 — the machine reports no counter frequency (the
    /// case `drivers::timer::init` refuses to start the system tick in), so
    /// no duration can be formed here either. Falls back to a bounded
    /// iteration count purely so a wedged controller cannot hang the machine
    /// for ever. This is the old, CPU-clock-dependent behaviour and is
    /// named as such: on a machine without a counter, the limit is not a
    /// time.
    Spins(u32),
}

impl Budget {
    /// Open a budget of `ms` milliseconds for the current wait.
    fn new(ms: u32) -> Self {
        let frq = cntfrq();
        if frq == 0 {
            return Budget::Spins(SPIN_LIMIT);
        }
        Budget::Ticks { start: cntpct(), limit: ticks_for_ms(frq, ms) }
    }

    /// Has the budget run out? Call it *after* testing the register, so a
    /// condition that is already satisfied never reports a timeout.
    fn expired(&mut self) -> bool {
        match self {
            Budget::Ticks { start, limit } => reached(*start, cntpct(), *limit),
            Budget::Spins(left) => {
                if *left == 0 {
                    true
                } else {
                    *left -= 1;
                    false
                }
            }
        }
    }
}

/// The wait phases of one sector operation. Each carries the timeout the
/// specification gives *that* phase — they are not the same number — and
/// which line a failure in it belongs to.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Phase {
    /// Command Inhibit (CMD) must be 0 before a command may be written
    /// (SD-HC §3.7.1.1 (1)).
    #[default]
    CmdInhibit,
    /// Command Inhibit (DAT) must be 0 before a command that uses the DAT
    /// line (SD-HC §3.7.1.1 (4)). What is actually being waited out here is
    /// the **previous** write's programming busy (audit §2.4).
    DatInhibit,
    /// Command Complete (SD-HC §3.7.1.2 (1)).
    CmdDone,
    /// Buffer Read Ready — the card's data has arrived in the FIFO.
    ReadRdy,
    /// Buffer Write Ready — the FIFO has room for the block.
    WriteRdy,
    /// Transfer Complete of a read.
    ReadDone,
    /// Transfer Complete of a write: *"this bit is set … when the last data
    /// is written to the SD card as specified by data length **and the busy
    /// signal released**"* (SD-HC §2.2.17 bit 01, write case) — i.e. the end
    /// of the card's programming time.
    WriteDone,
}

impl Phase {
    /// Number of phases — the width of the per-phase counters.
    pub const COUNT: usize = 7;

    /// Index into the per-phase counters / [`PHASE_NAMES`].
    const fn index(self) -> usize {
        self as usize
    }

    /// The timeout this phase is allowed, in milliseconds.
    const fn limit_ms(self) -> u32 {
        match self {
            // Card busy: the write-busy limit of SD Phys §4.6.2.2.
            Phase::DatInhibit | Phase::WriteDone => WRITE_BUSY_MS,
            // Card read latency: the 100 ms minimum of SD Phys §4.6.2.1.
            Phase::ReadRdy | Phase::ReadDone => READ_MS,
            // Controller handshakes: the wedged-controller backstop.
            Phase::CmdInhibit | Phase::CmdDone | Phase::WriteRdy => CTRL_MS,
        }
    }

    /// Does a failure in this phase leave a transaction on the DAT line?
    /// Those are aborted with CMD12 + both software resets (SD-HC §3.8); a
    /// bare command failure only resets the line that failed (§3.10.1).
    const fn on_dat_line(self) -> bool {
        matches!(
            self,
            Phase::DatInhibit
                | Phase::ReadRdy
                | Phase::WriteRdy
                | Phase::ReadDone
                | Phase::WriteDone
        )
    }

    /// The software resets SD-HC §3.10.1 / §3.8 ask for, given this phase
    /// and the error bits that were set (0 for a plain timeout).
    const fn resets(self, status: u32) -> u32 {
        // §3.10.1: reset the line the reported error belongs to.
        let mut bits = 0;
        if status & INT_ERR_CMD_MASK != 0 {
            bits |= C1_SRST_CMD;
        }
        if status & INT_ERR_DAT_MASK != 0 {
            bits |= C1_SRST_DATA;
        }
        if self.on_dat_line() {
            // §3.8: an aborted data transaction takes both resets.
            bits |= C1_SRST_CMD | C1_SRST_DATA;
        } else {
            // A command phase that failed without any error bit (a plain
            // timeout) still leaves the command circuit mid-command.
            bits |= C1_SRST_CMD;
        }
        bits
    }
}

/// Phase names for the diagnostics, in [`Phase`] order.
pub const PHASE_NAMES: [&str; Phase::COUNT] =
    ["cmd_inhibit", "dat_inhibit", "cmd_done", "read_rdy", "write_rdy", "read_done", "write_done"];

/// The error bits the driver counts by name, in [`ERROR_BIT_NAMES`] order.
const ERROR_BIT_MASKS: [u32; 7] = [
    INT_ERR_CTO,
    INT_ERR_CCRC,
    INT_ERR_CEND,
    INT_ERR_CBAD,
    INT_ERR_DTO,
    INT_ERR_DCRC,
    INT_ERR_DEND,
];

/// Names of the counted error bits, in [`ERROR_BIT_MASKS`] order.
pub const ERROR_BIT_NAMES: [&str; 7] = ["CTO", "CCRC", "CEND", "CBAD", "DTO", "DCRC", "DEND"];

// ── Error bookkeeping (audit finding 12: report the bits, do not just
// clear them) ────────────────────────────────────────────────────────
static ERR_LAST_STATUS: AtomicU32 = AtomicU32::new(0);
static ERR_LAST_PHASE: AtomicU32 = AtomicU32::new(0);
static ERR_BIT_COUNT: [AtomicU32; ERROR_BIT_MASKS.len()] =
    [const { AtomicU32::new(0) }; ERROR_BIT_MASKS.len()];
static ERR_OTHER: AtomicU32 = AtomicU32::new(0);
static ERR_IO_TOTAL: AtomicU32 = AtomicU32::new(0);
static ERR_TIMEOUT_TOTAL: AtomicU32 = AtomicU32::new(0);
static ERR_TIMEOUT_PHASE: [AtomicU32; Phase::COUNT] =
    [const { AtomicU32::new(0) }; Phase::COUNT];
static ERR_RECOVERIES: AtomicU32 = AtomicU32::new(0);
static ERR_NONRECOVERABLE: AtomicU32 = AtomicU32::new(0);
static ERR_R1_TOTAL: AtomicU32 = AtomicU32::new(0);
static ERR_R1_LAST: AtomicU32 = AtomicU32::new(0);
static ERR_R1_BIT_COUNT: [AtomicU32; R1_BIT_MASKS.len()] =
    [const { AtomicU32::new(0) }; R1_BIT_MASKS.len()];
/// Print failures on the console? Off while [`init`] probes the controller
/// bases — a slot without a card answers every response command with a
/// command timeout, and that is the probe doing its job.
static REPORT_ERRORS: AtomicBool = AtomicBool::new(false);

/// What the driver has seen go wrong — the counters audit finding 12 asks
/// for, so a caller can tell a timeout from a CRC error from a controller
/// data timeout instead of getting a bare `BlockError::Io`.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ErrorStats {
    /// `INTERRUPT` as it was read at the most recent failed wait (0 when
    /// the failure was a plain timeout with no error bit set).
    pub last_status: u32,
    /// The phase that failed most recently.
    pub last_phase: Phase,
    /// How often each named error bit was seen, in [`ERROR_BIT_NAMES`] order.
    pub bits: [u32; ERROR_BIT_MASKS.len()],
    /// Errors with none of the named bits set (current-limit, Auto-CMD,
    /// ADMA, tuning — SD-HC §2.2.18 bits 23-26).
    pub other: u32,
    /// Waits that ended on an error bit (`BlockError::Io`).
    pub io_errors: u32,
    /// Waits that ran out of time (`BlockError::Timeout`).
    pub timeouts: u32,
    /// Timeouts per phase, in [`PHASE_NAMES`] order.
    pub timeouts_by_phase: [u32; Phase::COUNT],
    /// Recovery sequences run (SD-HC §3.10.1 / §3.8).
    pub recoveries: u32,
    /// Recoveries after which Command Inhibit was still set — SD-HC
    /// §3.10.1's "Non-recoverable Error".
    pub nonrecoverable: u32,
    /// Commands the **card** rejected: R1 responses carrying at least one
    /// Type-`E` bit of SD Phys Table 4-42 (audit finding 3).
    pub card_errors: u32,
    /// The card status word of the most recent rejected command.
    pub last_card_status: u32,
    /// How often each named R1 error bit was seen, in [`R1_BIT_NAMES`] order.
    pub card_bits: [u32; R1_BIT_MASKS.len()],
}

/// A snapshot of the driver's error bookkeeping. Cheap: plain relaxed loads.
pub fn error_stats() -> ErrorStats {
    let mut s = ErrorStats {
        last_status: ERR_LAST_STATUS.load(Ordering::Relaxed),
        last_phase: PHASES[(ERR_LAST_PHASE.load(Ordering::Relaxed) as usize) % Phase::COUNT],
        other: ERR_OTHER.load(Ordering::Relaxed),
        io_errors: ERR_IO_TOTAL.load(Ordering::Relaxed),
        timeouts: ERR_TIMEOUT_TOTAL.load(Ordering::Relaxed),
        recoveries: ERR_RECOVERIES.load(Ordering::Relaxed),
        nonrecoverable: ERR_NONRECOVERABLE.load(Ordering::Relaxed),
        card_errors: ERR_R1_TOTAL.load(Ordering::Relaxed),
        last_card_status: ERR_R1_LAST.load(Ordering::Relaxed),
        ..ErrorStats::default()
    };
    for (i, c) in ERR_BIT_COUNT.iter().enumerate() {
        s.bits[i] = c.load(Ordering::Relaxed);
    }
    for (i, c) in ERR_R1_BIT_COUNT.iter().enumerate() {
        s.card_bits[i] = c.load(Ordering::Relaxed);
    }
    for (i, c) in ERR_TIMEOUT_PHASE.iter().enumerate() {
        s.timeouts_by_phase[i] = c.load(Ordering::Relaxed);
    }
    s
}

/// [`Phase`] by index — the inverse of [`Phase::index`].
const PHASES: [Phase; Phase::COUNT] = [
    Phase::CmdInhibit,
    Phase::DatInhibit,
    Phase::CmdDone,
    Phase::ReadRdy,
    Phase::WriteRdy,
    Phase::ReadDone,
    Phase::WriteDone,
];

#[inline(always)]
fn rd(off: usize) -> u32 {
    // SAFETY: MMIO read at a valid SD-host register address.
    unsafe { core::ptr::read_volatile((base() + off) as *const u32) }
}
#[inline(always)]
fn wr(off: usize, val: u32) {
    // SAFETY: MMIO write at a valid SD-host register address.
    unsafe { core::ptr::write_volatile((base() + off) as *mut u32, val) }
}

/// Extract CSD field `[hi:lo]` from the 4-word response. SDHCI stores a
/// 136-bit response right-shifted by 8 bits (the CRC byte is dropped),
/// so CSD bit `n` is at response bit `n-8`.
fn csd_bits(r: &[u32; 4], hi: u32, lo: u32) -> u32 {
    let mut v = 0u32;
    for b in lo..=hi {
        let rb = b - 8;
        v |= ((r[(rb / 32) as usize] >> (rb % 32)) & 1) << (b - lo);
    }
    v
}

/// Compute the card's 512-byte block count from its CSD (v1 SDSC or
/// v2 SDHC/SDXC).
fn parse_capacity(r: &[u32; 4]) -> u64 {
    if csd_bits(r, 127, 126) == 1 {
        // v2: capacity = (C_SIZE + 1) × 512 KiB.
        (csd_bits(r, 69, 48) as u64 + 1) * 1024
    } else {
        // v1: (C_SIZE+1) × 2^(C_SIZE_MULT+2) blocks × 2^READ_BL_LEN bytes.
        let read_bl_len = csd_bits(r, 83, 80);
        let c_size = csd_bits(r, 73, 62) as u64;
        let c_size_mult = csd_bits(r, 49, 47);
        let blocknr = (c_size + 1) * (1u64 << (c_size_mult + 2));
        blocknr * (1u64 << read_bl_len) / SECTOR_SIZE as u64
    }
}

/// The microSD device on EMMC2.
pub struct Emmc {
    rca: AtomicU32,
    sdhc: AtomicBool,
    ready: AtomicBool,
    blocks: AtomicU64,
}

/// Board-selected block device instance.
pub static SDCARD: Emmc = Emmc {
    rca: AtomicU32::new(0),
    sdhc: AtomicBool::new(false),
    ready: AtomicBool::new(false),
    blocks: AtomicU64::new(0),
};

/// Record a failed wait and say so on the console — audit finding 12: the
/// bits used to be cleared before anyone could see which error it was, which
/// is why the twelve failed writes of the Pi measurement could not be told
/// apart from a CRC error or a controller data timeout.
///
/// The counters always run; the console line waits until [`REPORT_ERRORS`]
/// is set, because during the base probe a slot **without** a card answers
/// every response command with `CTO` — that is the probe working, not a
/// fault. Whatever the identification hit is printed once by
/// [`report_controller`] instead.
fn note_failure(phase: Phase, status: u32, timed_out: bool) {
    ERR_LAST_STATUS.store(status, Ordering::Relaxed);
    ERR_LAST_PHASE.store(phase.index() as u32, Ordering::Relaxed);
    let report = REPORT_ERRORS.load(Ordering::Relaxed);
    if timed_out {
        ERR_TIMEOUT_TOTAL.fetch_add(1, Ordering::Relaxed);
        ERR_TIMEOUT_PHASE[phase.index()].fetch_add(1, Ordering::Relaxed);
        if report {
            crate::kprintln!(
                "[emmc] {} timed out after {} ms (INTERRUPT=0x{:08x}, STATUS=0x{:08x})",
                PHASE_NAMES[phase.index()],
                phase.limit_ms(),
                status,
                rd(STATUS),
            );
        }
        return;
    }

    ERR_IO_TOTAL.fetch_add(1, Ordering::Relaxed);
    let mut named = 0u32;
    for (i, mask) in ERROR_BIT_MASKS.iter().enumerate() {
        if status & mask != 0 {
            ERR_BIT_COUNT[i].fetch_add(1, Ordering::Relaxed);
            named |= mask;
        }
    }
    if status & INT_ERROR_MASK & !named != 0 {
        ERR_OTHER.fetch_add(1, Ordering::Relaxed);
    }
    if !report {
        return;
    }
    crate::kprint!("[emmc] {} error: INTERRUPT=0x{:08x}", PHASE_NAMES[phase.index()], status);
    for (i, mask) in ERROR_BIT_MASKS.iter().enumerate() {
        if status & mask != 0 {
            crate::kprint!(" {}", ERROR_BIT_NAMES[i]);
        }
    }
    crate::kprintln!("");
}

/// Evaluate an **R1 card status** word and report what the card said —
/// audit finding 3 (`docs/api/hw/emmc-audit.md` §4), the bit table is
/// `docs/hardware/emmc-sd.md` §5 (SD Phys 9.10 Table 4-42).
///
/// `cmd_index` is the SD command number, only used for the console line.
/// Returns `Err(BlockError::Card)` when any Type-`E` bit is set: the card
/// has refused or failed the command, whatever the controller's
/// `INTERRUPT` register says.
fn check_r1(cmd_index: u32, status: u32) -> Result<(), BlockError> {
    if status & R1_ERROR_MASK == 0 {
        return Ok(());
    }
    ERR_R1_TOTAL.fetch_add(1, Ordering::Relaxed);
    ERR_R1_LAST.store(status, Ordering::Relaxed);
    for (i, mask) in R1_BIT_MASKS.iter().enumerate() {
        if status & mask != 0 {
            ERR_R1_BIT_COUNT[i].fetch_add(1, Ordering::Relaxed);
        }
    }
    if REPORT_ERRORS.load(Ordering::Relaxed) {
        crate::kprint!("[emmc] CMD{} rejected by card: status=0x{:08x}", cmd_index, status);
        for (i, mask) in R1_BIT_MASKS.iter().enumerate() {
            if status & mask != 0 {
                crate::kprint!(" {}", R1_BIT_NAMES[i]);
            }
        }
        // CURRENT_STATE (bits 12:9) says which state the card was in when
        // it received the command — Table 4-42, entry 12:9.
        crate::kprintln!(" (state {})", (status >> 9) & 0xF);
    }
    Err(BlockError::Card)
}

impl Emmc {
    /// Poll until `(rd(off) & mask) == 0` within `phase`'s time budget.
    /// On expiry the failure is recorded and the controller is recovered
    /// (SD-HC §3.10.1) before `false` is returned.
    fn wait_clear(off: usize, mask: u32, phase: Phase) -> bool {
        let mut budget = Budget::new(phase.limit_ms());
        loop {
            if rd(off) & mask == 0 {
                return true;
            }
            if budget.expired() {
                note_failure(phase, rd(INTERRUPT), true);
                Self::recover(phase, 0);
                return false;
            }
            core::hint::spin_loop();
        }
    }

    /// Wait for `flag` in INTERRUPT (clearing it), or an error / timeout.
    ///
    /// Both failure exits go through [`Emmc::recover`] — SD-HC 3.00 §3.10.1
    /// (Error Interrupt Recovery) and §3.8 (Abort Transaction). Before that
    /// the timeout exit left the controller mid-transfer with the DAT line
    /// still its own, so the *next* command paid the remainder of the same
    /// busy (audit finding 2, §3.2).
    fn wait_interrupt(flag: u32, phase: Phase) -> Result<(), BlockError> {
        let mut budget = Budget::new(phase.limit_ms());
        loop {
            let i = rd(INTERRUPT);
            if i & INT_ERROR_MASK != 0 {
                wr(INTERRUPT, i); // clear (W1C) — the value is kept below
                note_failure(phase, i, false);
                Self::recover(phase, i);
                return Err(BlockError::Io);
            }
            if i & flag != 0 {
                wr(INTERRUPT, flag); // clear the bit we consumed
                return Ok(());
            }
            if budget.expired() {
                note_failure(phase, i, true);
                Self::recover(phase, 0);
                return Err(BlockError::Timeout);
            }
            core::hint::spin_loop();
        }
    }

    /// Put the controller straight after a failed wait — SD Host Controller
    /// Simplified Spec 3.00 §3.10.1 (Error Interrupt Recovery) and §3.8
    /// (Abort Transaction), as required by audit finding 2:
    ///
    /// 1. the error status has been read and cleared by the failing wait
    ///    (and kept, see [`note_failure`]); it is passed in as `status`,
    /// 2. a transaction still on the DAT line is aborted with CMD12,
    /// 3. the line(s) that failed are software-reset,
    /// 4. Command Inhibit must be clear again afterwards — if it is not,
    ///    the controller is blocked and that is reported as §3.10.1's
    ///    "Non-recoverable Error".
    fn recover(phase: Phase, status: u32) {
        Self::recover_inner(phase, status, phase.on_dat_line())
    }

    /// [`Emmc::recover`] with the abort command suppressed — for the case
    /// where the **card** rejected the command (its R1 carried an error
    /// bit) rather than the transfer breaking. The card never left transfer
    /// state, so there is no data transaction on its side to abort; all
    /// that is stuck is the controller's data state machine, and
    /// `Software Reset For DAT Line` is defined to clear exactly that:
    /// SD-HC 3.00 §2.2.15 lists "Buffer Data Port", "Buffer Read Enable",
    /// "Buffer Write Enable", "Command Inhibit (DAT)" and "DAT Line Active"
    /// among the bits it clears (`docs/hardware/emmc-sd.md` §1 CONTROL1).
    fn recover_no_abort(phase: Phase) {
        Self::recover_inner(phase, 0, false)
    }

    fn recover_inner(phase: Phase, status: u32, abort: bool) {
        ERR_RECOVERIES.fetch_add(1, Ordering::Relaxed);
        // §3.10.1 (4): save and clear the interrupt status. Saved already —
        // `note_failure` has it (finding 12) — so the W1C here loses
        // nothing and leaves no stale bit for the next command to trip on.
        wr(INTERRUPT, rd(INTERRUPT));
        if abort {
            Self::abort_transaction();
        }
        Self::soft_reset(phase.resets(status));

        let mut budget = Budget::new(RECOVERY_MS);
        loop {
            let s = rd(STATUS);
            if s & (SR_CMD_INHIBIT | SR_DAT_INHIBIT) == 0 {
                return;
            }
            if budget.expired() {
                ERR_NONRECOVERABLE.fetch_add(1, Ordering::Relaxed);
                crate::kprintln!(
                    "[emmc] recovery incomplete, STATUS=0x{:08x} — non-recoverable (SD-HC 3.00 §3.10.1)",
                    s,
                );
                return;
            }
            core::hint::spin_loop();
        }
    }

    /// Write the given `Software Reset For …` bits and wait for the
    /// controller to clear them again (SD-HC §2.2.15). A reset that does not
    /// complete is not reported here — the Command Inhibit check in
    /// [`Emmc::recover`] is what decides recoverable vs. not.
    ///
    /// **One bit per write.** SD-HC §3.10.1 lists the two resets as two
    /// steps ("Software Reset For CMD Line", then "Software Reset For DAT
    /// Line"), and so does the abort sequence of §3.8; issuing them in one
    /// write is an optimisation the specification never asks for. It is
    /// also one a host controller may not implement: QEMU's SDHCI model
    /// decodes the reset byte with an **exact-match** `switch`
    /// (`hw/sd/sdhci.c:1150-1170`, v11.0.2 — `case SDHC_RESET_CMD:` /
    /// `case SDHC_RESET_DATA:`), so a write with both bits set matches no
    /// case and resets *nothing*, silently: the bits live in the upper byte
    /// of `CLKCON`, which the model does not store, so they read back as 0
    /// and the wait below reports a completed reset that never happened.
    /// Driven: with both bits in one write the controller stayed at
    /// `STATUS=0x01ff0506` (Command Inhibit (DAT) + DAT Line Active) after
    /// every recovery and no further sector access succeeded.
    fn soft_reset(bits: u32) {
        for one in [C1_SRST_CMD, C1_SRST_DATA] {
            if bits & one == 0 {
                continue;
            }
            wr(CONTROL1, (rd(CONTROL1) & !(C1_SRST_CMD | C1_SRST_DATA)) | one);
            let mut budget = Budget::new(RECOVERY_MS);
            while rd(CONTROL1) & one != 0 {
                if budget.expired() {
                    break;
                }
                core::hint::spin_loop();
            }
        }
    }

    /// Issue CMD12 as the abort command of SD-HC §3.8.
    ///
    /// Deliberately not routed through [`Emmc::command`]: that would open a
    /// second full wait budget and re-enter recovery on failure. The outcome
    /// is not checked — the software resets in [`Emmc::recover`] put the
    /// controller straight either way; the abort is what tells the *card* to
    /// leave the data transfer.
    fn abort_transaction() {
        // An abort command may be issued while Command Inhibit (DAT) is
        // still set, but not while the CMD line itself is busy.
        let mut budget = Budget::new(RECOVERY_MS);
        while rd(STATUS) & SR_CMD_INHIBIT != 0 {
            if budget.expired() {
                return; // CMD line blocked — the resets are all that is left
            }
            core::hint::spin_loop();
        }
        wr(INTERRUPT, rd(INTERRUPT));
        wr(ARG1, 0);
        wr(CMDTM, STOP_TRANSMISSION);
        let mut budget = Budget::new(RECOVERY_MS);
        loop {
            let i = rd(INTERRUPT);
            if i & (INT_CMD_DONE | INT_ERROR_MASK) != 0 {
                wr(INTERRUPT, i);
                return;
            }
            if budget.expired() {
                return;
            }
            core::hint::spin_loop();
        }
    }

    /// Issue a command and return `RESP0`.
    fn command(&self, cmdtm: u32, arg: u32) -> Result<u32, BlockError> {
        if !Self::wait_clear(STATUS, SR_CMD_INHIBIT, Phase::CmdInhibit) {
            return Err(BlockError::Timeout);
        }
        wr(INTERRUPT, rd(INTERRUPT)); // clear any leftover status
        wr(ARG1, arg);
        wr(CMDTM, cmdtm);
        Self::wait_interrupt(INT_CMD_DONE, Phase::CmdDone)?;
        Ok(rd(RESP0))
    }

    /// An application command (`CMD55` then the ACMD).
    fn app_command(&self, cmdtm: u32, arg: u32) -> Result<u32, BlockError> {
        let rca = self.rca.load(Ordering::Relaxed) << 16;
        self.command(APP_CMD, rca)?;
        self.command(cmdtm, arg)
    }

    /// Reset the controller and bring the clock up to `target_hz`.
    fn reset_and_clock(target_hz: u32) -> Result<(), BlockError> {
        wr(CONTROL0, 0);
        wr(CONTROL1, C1_SRST_HC);
        for _ in 0..SPIN_LIMIT {
            if rd(CONTROL1) & C1_SRST_HC == 0 {
                break;
            }
            core::hint::spin_loop();
        }
        // Power the SD bus: CONTROL0 byte 1 = Power Control —
        // SD_BUS_POWER (bit 8) + voltage select 3.3 V (0b111 at bits 11:9).
        wr(CONTROL0, 0x0F00);
        Self::set_clock(target_hz)?;
        wr(IRPT_EN, 0); // poll, don't signal
        wr(IRPT_MASK, 0xFFFF_FFFF); // but record all status bits
        Ok(())
    }

    /// Program the SD clock divider for ~`target_hz`.
    fn set_clock(target_hz: u32) -> Result<(), BlockError> {
        // Base clock from CAPABILITIES[15:8] (MHz); fall back to 100 MHz.
        let base_mhz = (rd(CAPABILITIES) >> 8) & 0xFF;
        let base_hz = if base_mhz == 0 { 100_000_000 } else { base_mhz * 1_000_000 };
        // SDCLK = base / (2 * div); pick the smallest div that fits.
        let mut div = 1u32;
        while base_hz / (2 * div) > target_hz && div < 0x3FF {
            div += 1;
        }
        let freq8 = (div & 0xFF) << 8;
        let freq_ms2 = ((div >> 8) & 0x3) << 6;
        wr(CONTROL1, C1_CLK_INTLEN | freq8 | freq_ms2 | C1_TOUNIT_MAX);
        for _ in 0..SPIN_LIMIT {
            if rd(CONTROL1) & C1_CLK_STABLE != 0 {
                break;
            }
            core::hint::spin_loop();
        }
        wr(CONTROL1, rd(CONTROL1) | C1_CLK_EN);
        Ok(())
    }

    /// Full card init: reset, clock, then the SD command sequence.
    fn init_card(&self) -> Result<(), BlockError> {
        Self::reset_and_clock(400_000)?; // identification speed

        self.command(GO_IDLE, 0)?;

        // CMD8: voltage check. Success ⇒ SD v2 (high-capacity capable).
        let v2 = self.command(SEND_IF_COND, 0x1AA).is_ok();

        // ACMD41: wait for power-up; HCS set when v2.
        let hcs = if v2 { 0x4000_0000 } else { 0 };
        let mut ocr = 0;
        for _ in 0..SPIN_LIMIT {
            ocr = self.app_command(SD_SEND_OP_COND, hcs | 0x00FF_8000)?;
            if ocr & 0x8000_0000 != 0 {
                break; // power-up complete
            }
            core::hint::spin_loop();
        }
        if ocr & 0x8000_0000 == 0 {
            return Err(BlockError::Timeout);
        }
        self.sdhc.store(ocr & 0x4000_0000 != 0, Ordering::Relaxed);

        self.command(ALL_SEND_CID, 0)?;
        let rca = self.command(SEND_REL_ADDR, 0)? >> 16;
        self.rca.store(rca, Ordering::Relaxed);

        // CMD9 (SEND_CSD) is valid in stand-by state — read it before
        // selecting the card to learn the capacity.
        self.command(SEND_CSD, rca << 16)?;
        let csd = [rd(RESP0), rd(RESP1), rd(RESP2), rd(RESP3)];
        self.blocks.store(parse_capacity(&csd), Ordering::Relaxed);

        self.command(SELECT_CARD, rca << 16)?;

        if !self.sdhc.load(Ordering::Relaxed) {
            self.command(SET_BLOCKLEN, SECTOR_SIZE as u32)?;
        }

        // Switch to default speed (~25 MHz) for transfers.
        Self::set_clock(25_000_000)?;
        self.ready.store(true, Ordering::Release);
        Ok(())
    }

    /// The `ARG1` value that addresses block `lba` on this card.
    ///
    /// SDHC/SDXC take a **block number**, SDSC a **byte address**
    /// (`docs/hardware/emmc-sd.md` §3, `CCS`) — and both are a single
    /// 32-bit command argument. An `lba` whose address does not fit is
    /// refused here instead of being truncated: `(lba as u32) * 512`
    /// wrapped silently in a release build, which would have put the
    /// transfer on a **different, valid** sector of the card. Found while
    /// driving the finding-3 probe, which is the first caller that ever
    /// reached the arithmetic with a large `lba`.
    fn card_address(&self, lba: u64) -> Result<u32, BlockError> {
        if self.sdhc.load(Ordering::Relaxed) {
            u32::try_from(lba).map_err(|_| BlockError::OutOfRange)
        } else {
            u32::try_from(lba)
                .ok()
                .and_then(|b| b.checked_mul(SECTOR_SIZE as u32))
                .ok_or(BlockError::OutOfRange)
        }
    }

    /// Read one 512-byte block at `lba` into `buf` (PIO).
    fn read_one(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {
        if !self.ready.load(Ordering::Acquire) {
            return Err(BlockError::NotReady);
        }
        if !Self::wait_clear(STATUS, SR_DAT_INHIBIT, Phase::DatInhibit) {
            return Err(BlockError::Timeout);
        }
        let addr = self.card_address(lba)?;
        wr(BLKSIZECNT, (1 << 16) | SECTOR_SIZE as u32);
        // The card answers CMD17 with R1 **before** the data phase, so a
        // refusal (address past the end of the card, internal error) is
        // visible here — and only here (finding 3). A refused command has
        // no data phase: returning without aborting would leave the next
        // wait to run its full budget for data that never comes.
        let status = self.command(READ_SINGLE, addr)?;
        if let Err(e) = check_r1(17, status) {
            Self::recover_no_abort(Phase::ReadRdy);
            return Err(e);
        }
        Self::wait_interrupt(INT_READ_RDY, Phase::ReadRdy)?;
        for word in buf.chunks_exact_mut(4) {
            let w = rd(DATA);
            word.copy_from_slice(&w.to_le_bytes());
        }
        Self::wait_interrupt(INT_DATA_DONE, Phase::ReadDone)?;
        Ok(())
    }

    /// Write one 512-byte block at `lba` from `buf` (PIO).
    fn write_one(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {
        if !self.ready.load(Ordering::Acquire) {
            return Err(BlockError::NotReady);
        }
        if !Self::wait_clear(STATUS, SR_DAT_INHIBIT, Phase::DatInhibit) {
            return Err(BlockError::Timeout);
        }
        let addr = self.card_address(lba)?;
        wr(BLKSIZECNT, (1 << 16) | SECTOR_SIZE as u32);
        // Same as the read side: CMD24's R1 arrives before the data phase.
        // This is the check whose absence let a write past the end of the
        // card report success (audit finding 3, driven).
        let status = self.command(WRITE_SINGLE, addr)?;
        if let Err(e) = check_r1(24, status) {
            Self::recover_no_abort(Phase::WriteRdy);
            return Err(e);
        }
        Self::wait_interrupt(INT_WRITE_RDY, Phase::WriteRdy)?;
        for word in buf.chunks_exact(4) {
            wr(DATA, u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        }
        Self::wait_interrupt(INT_DATA_DONE, Phase::WriteDone)?;
        Ok(())
    }
}

impl BlockDevice for Emmc {
    fn block_count(&self) -> u64 {
        self.blocks.load(Ordering::Relaxed)
    }

    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {
        if !buf.len().is_multiple_of(SECTOR_SIZE) {
            return Err(BlockError::Unsupported);
        }
        for (i, sector) in buf.chunks_exact_mut(SECTOR_SIZE).enumerate() {
            self.read_one(lba + i as u64, sector)?;
        }
        Ok(())
    }

    fn write_blocks(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {
        if !buf.len().is_multiple_of(SECTOR_SIZE) {
            return Err(BlockError::Unsupported);
        }
        for (i, sector) in buf.chunks_exact(SECTOR_SIZE).enumerate() {
            self.write_one(lba + i as u64, sector)?;
        }
        Ok(())
    }
}

/// Initialise the microSD card. Probes EMMC2 (real RPi4 SD slot) then
/// EMMC (QEMU raspi4b), using whichever has a responding card. Returns
/// `false` if neither has a card (the caller then has no block device —
/// never a stubbed success).
///
/// # Safety
/// Boot CPU, single-threaded; the VideoCore firmware has finished using
/// the card.
pub unsafe fn init() -> bool {
    // Idempotent: once a card is up, re-initialising would needlessly
    // re-run the SD power-up sequence. Callers may invoke this from both
    // boot bring-up and the self-test.
    if SDCARD.ready.load(Ordering::Acquire) {
        return true;
    }
    for &candidate in &[EMMC2_BASE, EMMC_BASE] {
        BASE_ADDR.store(candidate, Ordering::Relaxed);
        // Skip a base with no SDHCI controller behind it.
        let ver = rd(SLOTISR_VER);
        if ver == 0 || ver == 0xFFFF_FFFF {
            continue;
        }
        if SDCARD.init_card().is_ok() {
            crate::kprintln!("[emmc] SD card on controller @ 0x{:08x}", candidate);
            report_controller();
            // The probe is over: from here a controller error is a real one
            // and gets a console line of its own.
            REPORT_ERRORS.store(true, Ordering::Relaxed);
            return true;
        }
    }
    false
}

/// One-shot boot diagnostic: the values that decide whether the bus is
/// clocked the way the specification requires — measurement **M1** of
/// `docs/api/hw/emmc-audit.md` §5, for the controller base actually in use.
///
/// **Reported, not acted on.** Whether the guessed 100 MHz fallback base
/// clock is right (audit finding 9) and whether the divider is the 8-bit or
/// the 10-bit kind (finding 10) are their own tickets; this print is what
/// settles them on real hardware, where QEMU cannot (QEMU wires its card to
/// the legacy EMMC, so its `CAPABILITIES` are not the Pi's EMMC2 ones).
fn report_controller() {
    let ver = rd(SLOTISR_VER);
    let caps = rd(CAPABILITIES);
    let c0 = rd(CONTROL0);
    let c1 = rd(CONTROL1);
    // SLOTISR_VER[23:16] = Specification Version Number (SD-HC 3.00
    // §2.2.28): 0x00 = 1.00, 0x01 = 2.00, 0x02 = 3.00. Below 0x02 the
    // clock divider is the 8-bit kind and only powers of two are legal
    // (§2.2.14) — which our divider arithmetic does not know (finding 10).
    let spec_ver = (ver >> 16) & 0xFF;
    // CAPABILITIES[15:8] = Base Clock Frequency For SD Clock, in MHz
    // (SD-HC §2.2.26). 0 means the host system has to tell the driver by
    // another method (§3.2.1) — the mailbox rates below are that method.
    let base_mhz = (caps >> 8) & 0xFF;
    // CONTROL1[15:8] (low) + [7:6] (high) = the 10-bit SDCLK divider;
    // SDCLK = base / (2 × div), a divider of 0 meaning "base clock".
    let div = ((c1 >> 8) & 0xFF) | (((c1 >> 6) & 0x3) << 8);
    let base_hz = if base_mhz == 0 { 100_000_000 } else { base_mhz * 1_000_000 };
    let sdclk = if div == 0 { base_hz } else { base_hz / (2 * div) };
    crate::kprintln!(
        "[emmc] M1 SLOTISR_VER=0x{:08x} (SDHCI spec 0x{:02x}) CAPABILITIES=0x{:08x} (base {} MHz{})",
        ver,
        spec_ver,
        caps,
        base_mhz,
        if base_mhz == 0 { ", guessed 100" } else { "" },
    );
    crate::kprintln!(
        "[emmc] M1 CONTROL0=0x{:08x} CONTROL1=0x{:08x} div={} -> SDCLK ~{} Hz",
        c0,
        c1,
        div,
        sdclk,
    );
    // Mailbox `get clock rate` (tag 0x00030002) for both EMMC clock ids —
    // `docs/hardware/emmc-sd.md` §2 ("clock id 1 = EMMC") and
    // `references/circle/include/circle/bcmpropertytags.h:182,186`
    // (`CLOCK_ID_EMMC = 1`, `CLOCK_ID_EMMC2 = 12`). 0 = the firmware has no
    // rate for that clock.
    let rate = |id| super::mailbox::get_clock_rate(id).unwrap_or(0);
    crate::kprintln!(
        "[emmc] M1 mailbox clock rate: EMMC(1)={} Hz EMMC2(12)={} Hz",
        rate(super::mailbox::CLOCK_ID_EMMC),
        rate(super::mailbox::CLOCK_ID_EMMC2),
    );

    // Whatever the identification itself ran into, once (the per-failure
    // console lines are off until here — see `note_failure`). On a machine
    // where the first probed base has no card this is where its expected
    // command timeouts show up, instead of scattered through the boot log.
    let s = error_stats();
    crate::kprint!(
        "[emmc] M1 during identification: {} error(s), {} timeout(s), {} recovery(ies)",
        s.io_errors,
        s.timeouts,
        s.recoveries,
    );
    for (i, n) in s.bits.iter().enumerate() {
        if *n != 0 {
            crate::kprint!(" {}={}", ERROR_BIT_NAMES[i], n);
        }
    }
    crate::kprintln!("");
}

#[cfg(test)]
mod tests {
    //! Pure timeout arithmetic. **These do not run in the workspace test
    //! command**: the `kernel` crate is pinned to
    //! `aarch64-unknown-none-softfloat` via `forced-target` and is not
    //! host-linkable (`tools/xtask/src/main.rs:573-582`), so
    //! `cargo run -p xtask -- test` cannot execute them. They are kept
    //! because they are the whole of the arithmetic that decides a timeout,
    //! and they were driven on the host as a verbatim copy while the change
    //! was written.
    use super::*;

    #[test]
    fn a_millisecond_is_the_counter_frequency_over_a_thousand() {
        // The two frequencies these boards report: 54 MHz (Pi 4, measured
        // in `measurements/measurement-2026-09-25-fs04`) and 62.5 MHz (QEMU virt).
        assert_eq!(ticks_for_ms(54_000_000, 1), 54_000);
        assert_eq!(ticks_for_ms(54_000_000, 1_000), 54_000_000);
        assert_eq!(ticks_for_ms(62_500_000, 100), 6_250_000);
        // …and the budgets the driver actually opens, in ticks:
        assert_eq!(ticks_for_ms(54_000_000, WRITE_BUSY_MS), 54_000_000);
        assert_eq!(ticks_for_ms(54_000_000, READ_MS), 10_800_000);
    }

    #[test]
    fn a_budget_is_never_zero_ticks() {
        // A counter slow enough that a millisecond is less than one tick
        // still gets a tick, so a wait polls at least once more.
        assert_eq!(ticks_for_ms(100, 1), 1);
        assert_eq!(ticks_for_ms(1, 1), 1);
    }

    #[test]
    fn the_budget_does_not_overflow_at_any_plausible_frequency() {
        // 1 s at 4 GHz still fits; nothing here can wrap into a short wait.
        assert_eq!(ticks_for_ms(4_000_000_000, 1_000), 4_000_000_000);
    }

    #[test]
    fn expiry_is_the_elapsed_difference_not_the_absolute_value() {
        assert!(!reached(1_000, 1_999, 1_000));
        assert!(reached(1_000, 2_000, 1_000));
        assert!(reached(1_000, 5_000, 1_000));
    }

    #[test]
    fn expiry_survives_the_counter_wrapping() {
        // A wait that starts 500 ticks below the wrap and runs 400 ticks
        // past it has taken 900 ticks, not a negative or gigantic number.
        let start = u64::MAX - 499; // 500 ticks before the counter wraps
        let now = 400u64; // 500 + 400 = 900 ticks later
        assert!(!reached(start, now, 1_000));
        assert!(reached(start, now, 900));
        assert!(reached(start, now, 100));
    }

    #[test]
    fn each_phase_gets_the_limit_its_own_spec_clause_gives_it() {
        // SD Phys 6.00 §4.6.2.2 (write busy) vs. §4.6.2.1 (read) — the two
        // are not the same number, which is the point of finding 1.
        assert_eq!(Phase::WriteDone.limit_ms(), WRITE_BUSY_MS);
        assert_eq!(Phase::DatInhibit.limit_ms(), WRITE_BUSY_MS);
        assert_eq!(Phase::ReadRdy.limit_ms(), READ_MS);
        assert_eq!(Phase::ReadDone.limit_ms(), READ_MS);
        assert_eq!(Phase::CmdDone.limit_ms(), CTRL_MS);
        assert_eq!(Phase::CmdInhibit.limit_ms(), CTRL_MS);
        assert_eq!(Phase::WriteRdy.limit_ms(), CTRL_MS);
        // The write budget must exceed the 500 ms the specification allows
        // the card for the last busy of a write.
        assert!(WRITE_BUSY_MS > 500);
        // …and the read budget must be at least the 100 ms minimum.
        assert!(READ_MS >= 100);
    }

    #[test]
    fn the_phase_table_and_the_names_line_up() {
        for (i, p) in PHASES.iter().enumerate() {
            assert_eq!(p.index(), i);
        }
        assert_eq!(PHASES.len(), Phase::COUNT);
        assert_eq!(PHASE_NAMES.len(), Phase::COUNT);
        assert_eq!(ERROR_BIT_NAMES.len(), ERROR_BIT_MASKS.len());
    }

    #[test]
    fn a_data_phase_takes_both_resets_a_command_phase_only_the_cmd_line() {
        // SD-HC §3.8: an aborted data transaction takes both software
        // resets; §3.10.1: a command-line error only the CMD line.
        assert_eq!(Phase::WriteDone.resets(0), C1_SRST_CMD | C1_SRST_DATA);
        assert_eq!(Phase::DatInhibit.resets(0), C1_SRST_CMD | C1_SRST_DATA);
        assert_eq!(Phase::CmdDone.resets(INT_ERR_CTO), C1_SRST_CMD);
        assert_eq!(Phase::CmdInhibit.resets(0), C1_SRST_CMD);
        // …unless a DAT-line error bit says otherwise.
        assert_eq!(Phase::CmdDone.resets(INT_ERR_DCRC), C1_SRST_CMD | C1_SRST_DATA);
    }

    #[test]
    fn the_error_masks_are_the_bits_the_audit_names() {
        // `docs/api/hw/emmc-audit.md` §5 M2: 16 CTO, 17 CCRC, 20 DTO,
        // 21 DCRC, 22 DEND.
        assert_eq!(INT_ERR_CTO, 1 << 16);
        assert_eq!(INT_ERR_CCRC, 1 << 17);
        assert_eq!(INT_ERR_DTO, 1 << 20);
        assert_eq!(INT_ERR_DCRC, 1 << 21);
        assert_eq!(INT_ERR_DEND, 1 << 22);
        assert_eq!(INT_ERR_CMD_MASK & INT_ERR_DAT_MASK, 0);
        assert_eq!((INT_ERR_CMD_MASK | INT_ERR_DAT_MASK) & !INT_ERROR_MASK, 0);
    }

    #[test]
    fn the_spin_fallback_is_bounded() {
        let mut b = Budget::Spins(3);
        assert!(!b.expired());
        assert!(!b.expired());
        assert!(!b.expired());
        assert!(b.expired());
        assert!(b.expired());
    }
}
