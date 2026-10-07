//! FS08 part 2 — the cluster chain is stepped, not re-walked
//! (`docs/api/gemdos/_state.md` §3a; EmuTOS `nextcl`, `bdos/fsfat.c:247-293`,
//! `ixlseek`, `bdos/fsio.c:354-426`).
//!
//! Counts FAT entry lookups ([`tos_api::bdos::fs::fat::fat_lookups`]): a
//! sequential read of an `n`-cluster file takes `n - 1` steps (one per new
//! cluster) whatever the `Fread` size; a forward `Fseek` continues from the
//! current cluster; only a backward `Fseek` starts again at the start
//! cluster. Before FS08 a read of `k` bytes per call walked
//! `pos / clsizb` links per call — quadratic in the file length
//! (`docs/analysis/fs08-sd-read-2026-10-07.md` §2.2: 4 746 240 lookups for
//! one 4 MiB benchmark).

mod fatclus;

use fatclus::*;
use tos_api::bdos::fs::fat::fat_lookups;

#[test]
fn sequential_reads_take_one_fat_step_per_cluster() {
    let (_g, _host) = fs();
    let clusters = SEQ_LEN.div_ceil(CLSIZB);
    for chunk in [100usize, 512, 1024, 4096, 65536] {
        let h = fopen("C:\\SEQ.DAT", 0);
        assert!(h >= 6);
        let before = fat_lookups();
        let got = read_all(h, chunk);
        let steps = fat_lookups() - before;
        assert_eq!(got.len(), SEQ_LEN as usize);
        // n - 1 links between n clusters; nothing more.
        assert_eq!(steps, clusters - 1, "{chunk}-byte Freads took {steps} FAT steps");
        assert_eq!(fclose(h), 0);
    }
}

#[test]
fn forward_seek_chains_on_backward_seek_restarts() {
    let (_g, _host) = fs();
    let h = fopen("C:\\SEQ.DAT", 0);
    assert!(h >= 6);
    let c = CLSIZB as i64;
    let mut b = [0u8; 16];

    // To the middle of cluster 20 (index), from the start: 20 steps.
    let before = fat_lookups();
    assert_eq!(fseek(20 * c + 5, h, 0), (20 * c + 5) as i32);
    assert_eq!(fat_lookups() - before, 20);
    assert_eq!(fread(h, &mut b), 16);
    assert_eq!(b[0], pattern(SEQ_SEED, 20 * 2048 + 5));

    // Forward to cluster 25: 5 more steps from the current cluster.
    let before = fat_lookups();
    assert_eq!(fseek(25 * c + 9, h, 0), (25 * c + 9) as i32);
    assert_eq!(fat_lookups() - before, 5);
    assert_eq!(fread(h, &mut b), 16);
    assert_eq!(b[0], pattern(SEQ_SEED, 25 * 2048 + 9));

    // Forward onto a cluster boundary: positioned one cluster short
    // (`bdos/fsio.c:410-411`), the read then steps once.
    let before = fat_lookups();
    assert_eq!(fseek(30 * c, h, 0), (30 * c) as i32);
    assert_eq!(fat_lookups() - before, 4);
    let before = fat_lookups();
    assert_eq!(fread(h, &mut b), 16);
    assert_eq!(fat_lookups() - before, 1);
    assert_eq!(b[0], pattern(SEQ_SEED, 30 * 2048));

    // Backwards: from the start cluster again.
    let before = fat_lookups();
    assert_eq!(fseek(3 * c + 1, h, 0), (3 * c + 1) as i32);
    assert_eq!(fat_lookups() - before, 3);
    assert_eq!(fread(h, &mut b), 16);
    assert_eq!(b[0], pattern(SEQ_SEED, 3 * 2048 + 1));

    // To 0: no walk at all (`bdos/fsio.c:363-367`).
    let before = fat_lookups();
    assert_eq!(fseek(0, h, 0), 0);
    assert_eq!(fat_lookups() - before, 0);
    assert_eq!(fclose(h), 0);
}

#[test]
fn sequential_writes_take_one_step_per_new_cluster() {
    let (_g, _host) = fs();
    let _ = fdelete("C:\\STEPW.DAT");
    let h = fcreate("C:\\STEPW.DAT");
    assert!(h >= 6);
    let data = vec![0x3Cu8; 512];
    let before = fat_lookups();
    for _ in 0..64 {
        assert_eq!(fwrite(h, &data), 512);
    }
    let steps = fat_lookups() - before;
    // 64 records = 16 clusters, allocated as EmuTOS `nextcl` + `findfree`
    // do it (`bdos/fsfat.c:247-293`, `:208-234`). The free clusters of the
    // fixture are 2..=9 and 50.. (10..=49 is SEQ.DAT, 60.. FRAG.DAT):
    //   cluster 2   first cluster: `findfree16` from the FAT start, no
    //               entry lookups (it scans the FAT sector directly);
    //   3..=9       per cluster: 1 lookup of the current link (end of chain)
    //               + `findfree(cl)` starting AT `cl` (used) then `cl+1`
    //               (free) = 3 lookups, 7 x 3 = 21;
    //   50          1 + `findfree(9)`: 9, then 10..=49 used (40), 50 free
    //               = 1 + 42 = 43;
    //   51..=57     7 x 3 = 21.
    // Total 85 — linear in the clusters written plus the scan over the used
    // run. Before FS08 each Fwrite also re-walked the chain from the start.
    assert_eq!(steps, 85, "64 sequential 512-byte Fwrites took {steps} FAT lookups");
    assert_eq!(fclose(h), 0);
    let h = fopen("C:\\STEPW.DAT", 0);
    assert_eq!(read_all(h, 4096), vec![0x3Cu8; 64 * 512]);
    assert_eq!(fclose(h), 0);
    assert_eq!(fdelete("C:\\STEPW.DAT"), 0);
}
