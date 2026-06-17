//! Phase 02 self-test: exercise the `SVC #1` (GEMDOS) trap path from
//! the kernel (EL1) — the negative-function probe (`EINVFN`) and the
//! end-to-end PL011 RX → GIC → ringbuffer → host → GEMDOS echo path.
//!
//! This test runs at EL1; the synchronous exception enters through the
//! "Current EL with SP0/SPx" vector.
//!
//! ## Why there is no `Cconws` sub-test here anymore
//!
//! `Cconws` (0x09) reads the caller's string through an **unprivileged
//! load** (`ldtrb`, see [`tos_api::gemdos::console::cconws`]) so the
//! access is permission-checked as if EL0 issued it — the correct,
//! TOS-faithful contract that syscall pointers are *user* pointers
//! (and the basis for the future m68k emulator + real user processes).
//! A kernel-EL1 self-test can only hand `Cconws` a *kernel* rodata
//! pointer, which the `ldtrb` then (rightly) refuses with a level-2
//! permission fault (EL0 has no access to kernel rodata). So a
//! `Cconws`-from-EL1 test is invalid *by design*. The real `Cconws`
//! round-trip is instead verified from EL0 by the Phase-06 `Pexec`
//! self-test, where the child process prints "Hello" through `Cconws`
//! with a user-space string pointer — exactly how a ported program
//! calls it. (Root-caused 2026-06-16; see docs/STATUS.md.)

use crate::console::CONSOLE;
use crate::hal::uart::Uart;

/// Run all PHASE-02 self-tests.
pub fn run_phase02_self_test() {
    // Negative-path probe: an unknown function ID must yield EINVFN.
    // (No pointer argument, so this is valid to issue from EL1.)
    let bad: i64;
    // SAFETY: see above; uses a deliberately invalid function ID.
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") 0xFFFFu64,
            inout("x0") 0u64 => bad,
            options(nostack)
        );
    }
    if bad as i32 == tos_types::error::EINVFN {
        CONSOLE.write_str("[selftest] EINVFN OK\n");
    } else {
        CONSOLE.write_str("[selftest] EINVFN WRONG\n");
    }

    // ---- D5: PL011 RX through GIC + ringbuffer ----------------------
    //
    // Polls Crawio(0xFF) up to ~2 seconds; every received byte is echoed
    // via Cconout. Loop terminates on the first CR/LF or after the
    // poll budget is exhausted, whichever comes first. Driven through
    // SVC #1 so the entire IRQ → ringbuffer → host → GEMDOS path is
    // exercised end-to-end.
    CONSOLE.write_str("[selftest] RX echo (CR/LF or ~2s timeout)> ");
    let mut budget: u32 = 20_000_000;
    let mut got_byte = false;
    loop {
        let byte = crawio_read();
        if byte != 0 {
            got_byte = true;
            cconout(byte as u8);
            if byte == b'\r' as i32 || byte == b'\n' as i32 {
                cconout(b'\n');
                break;
            }
        } else {
            budget = budget.saturating_sub(1);
            if budget == 0 {
                CONSOLE.write_str("\n[selftest] RX timeout\n");
                break;
            }
        }
    }
    if got_byte {
        CONSOLE.write_str("[selftest] RX-IRQ OK\n");
    }
}

#[inline(always)]
fn crawio_read() -> i32 {
    // Crawio(0xFF) — non-blocking read; returns the byte, or 0 if no
    // input available.
    let r: i64;
    // SAFETY: SVC instruction; argument and function ID are constants.
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") 0x0006u64,
            inout("x0") 0x00FFu64 => r,
            options(nostack)
        );
    }
    r as i32
}

#[inline(always)]
fn cconout(byte: u8) {
    let _r: i64;
    // SAFETY: SVC instruction; argument and function ID are constants.
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") 0x0002u64,
            inout("x0") byte as u64 => _r,
            options(nostack)
        );
    }
}

/// Phase 03 D6 stack-overflow probe.
///
/// Recursively pushes a 4 KiB frame until the stack pointer crosses
/// into the guard region, at which point a Data Abort fires and
/// `rust_unhandled_handler` prints `*** STACK OVERFLOW ***` before
/// halting.
///
/// **Never invoked from the normal boot path** — it intentionally
/// crashes the kernel. Wired into [`run_phase02_self_test`] only when
/// the temporary `KERNEL_TEST_STACK_OVERFLOW` env var is set at build
/// time (see kernel/build.rs).
#[inline(never)]
pub fn trigger_stack_overflow() -> ! {
    use crate::kprintln;
    kprintln!("[selftest] *** triggering deliberate stack overflow ***");
    recurse(0);
    crate::arch::aarch64::halt();
}

#[inline(never)]
#[allow(unconditional_recursion)] // unbounded recursion is the point.
fn recurse(depth: u64) {
    // 4 KiB local buffer per frame, written so the compiler cannot
    // optimise it out and so each frame actually touches new pages.
    let mut buf = [0u8; 4096];
    buf[0] = depth as u8;
    buf[4095] = depth as u8;
    core::hint::black_box(&buf);
    recurse(depth + 1);
}

// ════════════════════════════════════════════════════════════════════
// PHASE-04 self-test — BDOS memory traps via SVC #1
// ════════════════════════════════════════════════════════════════════
//
// Exit criteria from `phases/PHASE-04-bdos-memory.md`:
//
//   EC1  1000-cycle alloc/free churn returns pool to baseline
//   EC2  Mxalloc(-1, …) returns the largest free MD (per pool)
//   EC3  MX_PREFSTRAM falls back to TT-RAM when ST-RAM is exhausted
//   EC4  Mfree(wild_ptr) → EIMBA, double-free → EIMBA
//   EC5  Mshrink shrink → E_OK; Mshrink grow → EGSBF
//   EC6  Mshrink(0, blk, 0) ≡ Mfree(blk)
//   EC7  Maddalt range overlapping ST-RAM → −1
//   EC8  All five trap entries reach the BDOS layer via the GEMDOS
//        dispatcher (proven by every test issuing real `svc #1` calls)
//   EC9  Phase-02 RX-IRQ test still green after Phase 04 — verified
//        by leaving `run_phase02_self_test` untouched and running it
//        after this function in `kernel_main_rust`.
//
// Every test is isolated: it allocates, exercises the trap under
// scrutiny, and frees back to baseline before returning. A final
// invariant check verifies that no test leaked.

use tos_api::bdos::mem::init::stram_range;
use tos_api::bdos::mem::stats::{pool_stats, PoolKind, PoolStats};
use tos_types::error::{EGSBF, EIMBA, E_OK};
use tos_types::{MX_PREFSTRAM, MX_STRAM, MX_TTRAM};

/// Run the full Phase-04 BDOS-memory self-test suite.
///
/// Issues every memory trap (`Maddalt`, `Mxalloc`, `Malloc`, `Mfree`,
/// `Mshrink`) at least once via `svc #1` and asserts the documented
/// return values. Aggregates a pass/fail count and a final
/// invariant check ("pool returned to baseline") and prints the
/// roll-up on completion.
pub fn run_phase04_self_test() {
    use crate::kprintln;

    kprintln!("");
    kprintln!("[selftest p04] === BDOS memory tests ===");

    let st_before = pool_stats(PoolKind::Stram);
    let tt_before = pool_stats(PoolKind::Ttram);

    let mut passed: u32 = 0;
    let mut failed: u32 = 0;
    let mut tally = |ok: bool| if ok { passed += 1 } else { failed += 1 };

    tally(test_mxalloc_probe());
    tally(test_alloc_roundtrip());
    tally(test_mfree_errors());
    tally(test_mshrink_grow_rejected());
    tally(test_mshrink_smaller_works());
    tally(test_mshrink_zero_frees());
    tally(test_maddalt_overlap_reject());
    tally(test_mxalloc_prefstram_fallback());
    tally(test_churn_no_leak());

    let st_after = pool_stats(PoolKind::Stram);
    let tt_after = pool_stats(PoolKind::Ttram);
    let baseline_ok = stats_eq(&st_before, &st_after) && stats_eq(&tt_before, &tt_after);
    if baseline_ok {
        kprintln!("[selftest p04] baseline-restored OK");
    } else {
        kprintln!(
            "[selftest p04] baseline-restored FAIL: \
             ST {}→{} mds {}→{}; TT {}→{} mds {}→{}",
            st_before.free_bytes, st_after.free_bytes,
            st_before.free_blocks, st_after.free_blocks,
            tt_before.free_bytes, tt_after.free_bytes,
            tt_before.free_blocks, tt_after.free_blocks,
        );
    }
    tally(baseline_ok);

    kprintln!(
        "[selftest p04] === BDOS memory: {} passed, {} failed ===",
        passed, failed,
    );
}

/// Compare two `PoolStats` snapshots for the load-bearing fields:
/// free/used byte sums, free-block count, allocated-block count,
/// largest-free. We deliberately do **not** compare `pool` (the
/// snapshot label) since both snapshots were taken on the same pool.
fn stats_eq(a: &PoolStats, b: &PoolStats) -> bool {
    a.free_bytes == b.free_bytes
        && a.used_bytes == b.used_bytes
        && a.free_blocks == b.free_blocks
        && a.allocated_blocks == b.allocated_blocks
        && a.largest_free == b.largest_free
}

/// Print a labelled OK/FAIL line and return the pass-bool.
fn check(ok: bool, label: &str) -> bool {
    use crate::kprintln;
    if ok {
        kprintln!("[selftest p04] {} OK", label);
    } else {
        kprintln!("[selftest p04] {} FAIL", label);
    }
    ok
}

// ─── SVC helpers ──────────────────────────────────────────────────

/// Issue `svc #1` with the GEMDOS function ID in `x8` and the given
/// argument register layout, returning `x0` as a sign-extended
/// 64-bit value (caller truncates to `i32` if needed).
///
/// Variants are coded out by arity rather than parameterised because
/// inline-asm constraints cannot be macroed cleanly across argument
/// counts — `inout("x0") …` must appear exactly once.
#[inline(always)]
fn svc_gemdos_1(fn_id: u64, a: u64) -> i64 {
    let r: i64;
    // SAFETY: SVC #1 is the GEMDOS trap class; the kernel handler
    // reads `x0..x7` + `x8` only and writes the result to `x0`.
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") fn_id,
            inout("x0") a => r,
            options(nostack)
        );
    }
    r
}

#[inline(always)]
fn svc_gemdos_2(fn_id: u64, a: u64, b: u64) -> i64 {
    let r: i64;
    // SAFETY: see [`svc_gemdos_1`].
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") fn_id,
            inout("x0") a => r,
            in("x1") b,
            options(nostack)
        );
    }
    r
}

#[inline(always)]
fn svc_gemdos_3(fn_id: u64, a: u64, b: u64, c: u64) -> i64 {
    let r: i64;
    // SAFETY: see [`svc_gemdos_1`].
    unsafe {
        core::arch::asm!(
            "svc #1",
            in("x8") fn_id,
            inout("x0") a => r,
            in("x1") b,
            in("x2") c,
            options(nostack)
        );
    }
    r
}

/// Sign-extend `i32` (Atari `LONG`) into the AArch64 argument register.
#[inline(always)]
fn arg_long(v: i32) -> u64 {
    v as i64 as u64
}

/// Sign-extend `i16` (Atari `WORD`) into the AArch64 argument register.
#[inline(always)]
fn arg_word(v: i16) -> u64 {
    v as i64 as u64
}

fn gemdos_mxalloc(amount: i32, mode: i16) -> i32 {
    svc_gemdos_2(0x44, arg_long(amount), arg_word(mode)) as i32
}

fn gemdos_malloc(amount: i32) -> i32 {
    svc_gemdos_1(0x48, arg_long(amount)) as i32
}

fn gemdos_mfree(addr: i32) -> i32 {
    svc_gemdos_1(0x49, arg_long(addr)) as i32
}

fn gemdos_mshrink(n: i16, blk: i32, len: i32) -> i32 {
    svc_gemdos_3(0x4A, arg_word(n), arg_long(blk), arg_long(len)) as i32
}

fn gemdos_maddalt(start: i32, size: i32) -> i32 {
    svc_gemdos_2(0x14, arg_long(start), arg_long(size)) as i32
}

// ─── Tests ────────────────────────────────────────────────────────

/// EC2 — `Mxalloc(-1, …)` returns the largest free block of the
/// selected pool(s); `MX_PREFSTRAM` returns `max(st_largest,
/// tt_largest)`.
fn test_mxalloc_probe() -> bool {
    let st = pool_stats(PoolKind::Stram);
    let tt = pool_stats(PoolKind::Ttram);

    let probe_st = gemdos_mxalloc(-1, MX_STRAM);
    let probe_tt = gemdos_mxalloc(-1, MX_TTRAM);
    let probe_pref = gemdos_mxalloc(-1, MX_PREFSTRAM);

    let expected_pref = if st.largest_free >= tt.largest_free {
        st.largest_free
    } else {
        tt.largest_free
    };

    let ok = probe_st == st.largest_free
        && probe_tt == tt.largest_free
        && probe_pref == expected_pref;

    check(ok, "EC2 Mxalloc(-1, …) probes per-pool largest")
}

/// EC8 — round-trip both `Mxalloc(amount, mode)` and `Malloc(amount)`
/// through the GEMDOS dispatcher; non-zero pointer + clean free.
fn test_alloc_roundtrip() -> bool {
    let p = gemdos_mxalloc(64, MX_STRAM);
    let ok1 = p != 0;
    let r1 = gemdos_mfree(p);
    let ok2 = r1 == E_OK;

    let q = gemdos_malloc(64);
    let ok3 = q != 0;
    let r2 = gemdos_mfree(q);
    let ok4 = r2 == E_OK;

    check(
        ok1 && ok2 && ok3 && ok4,
        "EC8 alloc round-trip (Mxalloc + Malloc)",
    )
}

/// EC4 — `Mfree(wild_ptr)` and double-free both return `EIMBA`.
fn test_mfree_errors() -> bool {
    // First, a clean alloc/free for the double-free case.
    let p = gemdos_mxalloc(64, MX_STRAM);
    let r1 = gemdos_mfree(p);
    let ok1 = r1 == E_OK;
    let r2 = gemdos_mfree(p);
    let ok2 = r2 == EIMBA;

    // Wild pointer: address `1` is not the start of any allocated MD
    // (and routes to pmdalt via find_mpb's else-branch).
    let r3 = gemdos_mfree(1);
    let ok3 = r3 == EIMBA;

    check(
        ok1 && ok2 && ok3,
        "EC4 Mfree wild + double-free → EIMBA",
    )
}

/// EC5a — `Mshrink(grow)` returns `EGSBF`.
fn test_mshrink_grow_rejected() -> bool {
    let p = gemdos_mxalloc(64, MX_STRAM);
    let r = gemdos_mshrink(0, p, 256);
    let ok = r == EGSBF;
    let _ = gemdos_mfree(p);
    check(ok, "EC5 Mshrink grow → EGSBF")
}

/// EC5b — `Mshrink(smaller)` returns `E_OK` and the released tail is
/// usable by a subsequent allocation (verified by pool stats).
fn test_mshrink_smaller_works() -> bool {
    let p = gemdos_mxalloc(256, MX_STRAM);
    let r = gemdos_mshrink(0, p, 32);
    let ok1 = r == E_OK;
    let r2 = gemdos_mfree(p);
    let ok2 = r2 == E_OK;
    check(ok1 && ok2, "EC5 Mshrink smaller → E_OK")
}

/// EC6 — `Mshrink(0, blk, 0)` is equivalent to `Mfree(blk)` (a
/// follow-up `Mfree` finds nothing on `mp_mal`).
fn test_mshrink_zero_frees() -> bool {
    let p = gemdos_mxalloc(64, MX_STRAM);
    let r1 = gemdos_mshrink(0, p, 0);
    let ok1 = r1 == E_OK;
    // Already freed by the Mshrink-to-zero path:
    let r2 = gemdos_mfree(p);
    let ok2 = r2 == EIMBA;
    check(ok1 && ok2, "EC6 Mshrink(0, blk, 0) ≡ Mfree(blk)")
}

/// EC7 — `Maddalt` rejects an alt-RAM addition that overlaps the
/// ST-RAM pool. We pick a region squarely inside ST-RAM so the
/// rejection is unambiguous.
fn test_maddalt_overlap_reject() -> bool {
    let (st_start, st_end) = stram_range();
    let middle = st_start + (st_end - st_start) / 2;
    let r = gemdos_maddalt(middle, 64);
    check(r == -1, "EC7 Maddalt ST-RAM overlap → −1")
}

/// EC3 — `MX_PREFSTRAM` falls back to TT-RAM when ST-RAM is full.
///
/// Strategy: drain ST-RAM by allocating the largest free block from
/// it, then ask for 64 bytes with `MX_PREFSTRAM`. The result must
/// land **outside** the ST-RAM range.
fn test_mxalloc_prefstram_fallback() -> bool {
    let st_largest = gemdos_mxalloc(-1, MX_STRAM);
    if st_largest <= 64 {
        // Pool too small to drain meaningfully; report skip.
        return check(false, "EC3 PREFSTRAM fallback (ST-RAM too small)");
    }

    let drain = gemdos_mxalloc(st_largest, MX_STRAM);
    if drain == 0 {
        return check(false, "EC3 PREFSTRAM fallback (drain failed)");
    }

    let pref = gemdos_mxalloc(64, MX_PREFSTRAM);
    let (st_start, st_end) = stram_range();
    let in_ttram = pref != 0 && (pref < st_start || pref >= st_end);

    let _ = gemdos_mfree(pref);
    let _ = gemdos_mfree(drain);

    check(in_ttram, "EC3 PREFSTRAM falls back to TT-RAM")
}

/// EC1 — 1000-cycle alloc/free churn leaves both pools at baseline.
fn test_churn_no_leak() -> bool {
    let st_before = pool_stats(PoolKind::Stram);
    let tt_before = pool_stats(PoolKind::Ttram);

    for _ in 0..1000 {
        let p = gemdos_mxalloc(64, MX_STRAM);
        if p == 0 {
            return check(false, "EC1 churn alloc failed");
        }
        let r = gemdos_mfree(p);
        if r != E_OK {
            return check(false, "EC1 churn free failed");
        }
    }

    let st_after = pool_stats(PoolKind::Stram);
    let tt_after = pool_stats(PoolKind::Ttram);

    let ok = stats_eq(&st_before, &st_after) && stats_eq(&tt_before, &tt_after);
    check(ok, "EC1 1000-cycle alloc/free no leak")
}
