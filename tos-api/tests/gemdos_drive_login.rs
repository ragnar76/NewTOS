//! What a GEMDOS file-system call pays to find out which drive it is on.
//!
//! **Where this comes from.** The per-opcode means in
//! `tosdump/reference/TERADESK-GDTRACE-r1.BIN` (Pi 4, TeraDesk opening a
//! directory window):
//!
//! ```text
//!    op name         calls  failed   total_us   mean_us
//!    4f Fsnext         229       5     118307     516.6
//!    4e Fsfirst          8       3       5140     642.5
//!    3d Fopen            2       1       1033     516.5
//!    47 Dgetpath         2       0       1025     512.5
//!    3b Dsetpath         1       0        513     513.0
//!    1a Fsetdta         16       0          0       0.0
//!    2f Fgetdta          8       0          1       0.1
//!    19 Dgetdrv          6       0          0       0.0
//!    42 Fseek            2       0          0       0.0
//! ```
//!
//! The calls that mean ~512 us are exactly the ones that resolve a drive; the
//! ones that mean 0 are exactly the ones that do not. `Dgetpath` is the clean
//! measurement: with the drive table already in memory
//! (`bios/drive.rs`, no I/O) its only device access is `Volume::mount`, which
//! called `bpb::geometry` — an uncached `read_blocks` of the partition's boot
//! sector — on **every** call. One 512-byte single-block PIO read on this
//! board is that ~512 us; `Fread` in the same trace moves 13269 bytes in
//! 16822 us, i.e. ~647 us per sector, on the same order.
//!
//! **What EmuTOS does.** `ckdrv` (`bdos/fsdrive.c:77-105`) calls `Getbpb` only
//! while the drive's bit in `drvsel` is clear; once the drive is logged in,
//! `drvtbl[d]` answers and no sector is read. The bit is cleared again only on
//! a media change (`bdos/bdosmain.c:456-482`).
//!
//! 229 `Fsnext` calls at ~512 us of boot-sector re-read each is 117 ms of the
//! window-opening time in that trace.

mod fatvol;

use fatvol::*;
use std::sync::atomic::Ordering;
// ── 2. logging the drive in costs one boot-sector read, not one per call ──

#[test]
fn a_logged_in_drive_does_not_re_read_its_boot_sector() {
    let (_g, h) = fs();

    // Warm the drive up: whatever the first call costs, it is the log-in.
    let mut dta = Dta::new();
    assert_eq!(gemdos1(FSETDTA, dta.ptr()), E_OK);
    let star = cpath("C:\\*.*");
    assert_eq!(gemdos2(FSFIRST, star.as_ptr() as u64, FA_ANY), E_OK);

    let before = h.disk.boot_reads.load(Ordering::Relaxed);

    // The shape of the traced window read: one Fsfirst and a run of Fsnext,
    // plus the Dgetpath / Fopen TeraDesk mixes in.
    let mut buf = [0u8; 128];
    for _ in 0..8 {
        assert_eq!(gemdos2(FSFIRST, star.as_ptr() as u64, FA_ANY), E_OK);
        while gemdos1(FSNEXT, 0) == E_OK {}
        assert_eq!(gemdos2(DGETPATH, buf.as_mut_ptr() as u64, 0), E_OK);
        let hello = cpath("C:\\HELLO.TXT");
        let fh = gemdos2(FOPEN, hello.as_ptr() as u64, 0);
        assert!(fh >= 6);
        assert_eq!(gemdos1(FCLOSE, fh as u64), E_OK);
    }

    let after = h.disk.boot_reads.load(Ordering::Relaxed);
    assert_eq!(
        after - before,
        0,
        "a logged-in drive must not re-read its boot sector: EmuTOS `ckdrv` \
         calls `Getbpb` only while the drive's `drvsel` bit is clear \
         (bdos/fsdrive.c:94-105); {} extra reads is one per GEMDOS call",
        after - before
    );
}

#[test]
fn a_partition_rescan_logs_every_drive_out_again() {
    let (_g, h) = fs();

    let mut dta = Dta::new();
    assert_eq!(gemdos1(FSETDTA, dta.ptr()), E_OK);
    let star = cpath("C:\\*.*");
    assert_eq!(gemdos2(FSFIRST, star.as_ptr() as u64, FA_ANY), E_OK);

    // A fresh partition scan can move or replace every partition, so the
    // cached geometry has to go (EmuTOS clears `drvsel` and frees the DMD on a
    // media change, `bdos/bdosmain.c:456-482`).
    assert_eq!(tos_api::bios::mount(), 1);

    let before = h.disk.boot_reads.load(Ordering::Relaxed);
    assert_eq!(gemdos2(FSFIRST, star.as_ptr() as u64, FA_ANY), E_OK);
    let after = h.disk.boot_reads.load(Ordering::Relaxed);
    assert_eq!(
        after - before,
        1,
        "the first access after a rescan must log the drive in again"
    );
}
