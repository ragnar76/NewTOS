//! FS08 part 2 — the GEMDOS read/write path moves data like EmuTOS `xrw`
//! (`docs/api/gemdos/_state.md` §3a, §3).
//!
//! Driven through the GEMDOS dispatcher against a FAT16 volume with 4-sector
//! clusters whose block device logs every request (`tests/fatclus`):
//!
//! * whole records go to/from the caller's buffer in **one request per run
//!   of contiguous clusters** (EmuTOS `xrw_recs` + `usrio`,
//!   `bdos/fsio.c:84-187`, `:57-74`), only partial records through the cache;
//! * a sequential read touches each FAT sector **once** — the FAT has its own
//!   buffer list that data traffic cannot evict (`bdos/fsbuf.c:67-71`, `:121`)
//!   and the position keeps the current cluster (`o_curcl`);
//! * `Fseek` forward/backward and reads across sector and cluster boundaries
//!   return the right bytes; writes read back, and both FAT copies agree.
//!
//! Before FS08 every sector was its own request and `Fread` walked the chain
//! from the start each call (`docs/analysis/fs08-sd-read-2026-10-07.md` §2.2).

mod fatclus;

use fatclus::*;

fn data_reads(log: &[Req]) -> Vec<Req> {
    log.iter().copied().filter(|r| !r.write && is_data(r.lba)).collect()
}

fn fat_reads(log: &[Req]) -> Vec<Req> {
    log.iter().copied().filter(|r| !r.write && is_fat(r.lba)).collect()
}

#[test]
fn one_fread_of_a_fragmented_file_is_one_request_per_contiguous_run() {
    let (_g, host) = fs();
    let h = fopen("C:\\FRAG.DAT", 0);
    assert!(h >= 6, "Fopen -> {h}");
    let mut buf = vec![0u8; FRAG_LEN as usize + 1000];
    host.clear_log();
    assert_eq!(fread(h, &mut buf), FRAG_LEN as i32);
    let log = host.log();
    assert_eq!(&buf[..FRAG_LEN as usize], &expected(FRAG_SEED, FRAG_LEN)[..]);

    // FRAG_LEN = 8 clusters - 100 bytes = 31 whole records + 412 bytes.
    // Runs [60 61] [90] [62] [120 121 122], then the 3 whole records of the
    // last cluster (70) as the 'tail records', then 412 bytes through the
    // cache (record 3 of cluster 70) — unless that record is cached already.
    let want = [
        Req { write: false, lba: cluster_lba(60), sectors: 8 },
        Req { write: false, lba: cluster_lba(90), sectors: 4 },
        Req { write: false, lba: cluster_lba(62), sectors: 4 },
        Req { write: false, lba: cluster_lba(120), sectors: 12 },
        Req { write: false, lba: cluster_lba(70), sectors: 3 },
    ];
    let data = data_reads(&log);
    assert_eq!(&data[..5], &want[..], "whole records: one request per run, got {data:?}");
    let rest = &data[5..];
    assert!(
        rest.is_empty() || rest == [Req { write: false, lba: cluster_lba(70) + 3, sectors: 1 }],
        "only the partial last record may go through the cache, got {rest:?}"
    );
    assert!(fat_reads(&log).len() <= 1, "one FAT sector covers the chain: {log:?}");
    assert_eq!(fclose(h), 0);
}

#[test]
fn sequential_small_freads_read_each_fat_sector_once() {
    let (_g, host) = fs();
    for chunk in [512usize, 100, 1024, 3000] {
        let h = fopen("C:\\SEQ.DAT", 0);
        assert!(h >= 6);
        host.clear_log();
        let got = read_all(h, chunk);
        let log = host.log();
        assert_eq!(got, expected(SEQ_SEED, SEQ_LEN), "contents with {chunk}-byte Freads");
        // All 40 clusters' entries live in FAT sector 0: at most one device
        // read of it for the whole file, however small the Freads. Before
        // FS08 data sectors pushed it out of the shared cache and every
        // chain walk read it again.
        let fat = fat_reads(&log);
        assert!(fat.len() <= 1, "{chunk}-byte Freads: FAT read {} times: {fat:?}", fat.len());
        // Every data sector of the file is read once at most.
        let sectors: u32 = data_reads(&log).iter().map(|r| r.sectors).sum();
        assert!(
            sectors <= SEQ_LEN.div_ceil(512),
            "{chunk}-byte Freads: {sectors} data sectors for a {}-sector file",
            SEQ_LEN.div_ceil(512)
        );
        assert_eq!(fclose(h), 0);
    }
}

#[test]
fn whole_record_freads_go_straight_to_the_device_in_one_request() {
    let (_g, host) = fs();
    let h = fopen("C:\\SEQ.DAT", 0);
    assert!(h >= 6);
    // 64 KiB from offset 0: 32 contiguous clusters, 128 records, one request.
    let mut buf = vec![0u8; 65536];
    host.clear_log();
    assert_eq!(fread(h, &mut buf), 65536);
    assert_eq!(&buf[..], &expected(SEQ_SEED, 65536)[..]);
    let data = data_reads(&host.log());
    assert_eq!(data, vec![Req { write: false, lba: cluster_lba(10), sectors: 128 }]);
    assert_eq!(fclose(h), 0);
}

#[test]
fn fseek_forward_backward_and_boundaries() {
    let (_g, _host) = fs();
    let all = expected(FRAG_SEED, FRAG_LEN);
    let h = fopen("C:\\FRAG.DAT", 0);
    assert!(h >= 6);
    let c = CLSIZB as i64;
    // (offset, length) pairs: cluster boundaries, one byte before/after,
    // sector boundaries, backwards jumps, the last byte.
    let probes: &[(i64, usize)] = &[
        (0, 10),
        (c, 600),
        (c - 1, 2),
        (3 * c + 512, 1500),
        (100, 5000),
        (2 * c, 2048),
        (7 * c + 3, 5000),
        (c * 5 - 512, 1024),
        (FRAG_LEN as i64 - 1, 10),
        (0, FRAG_LEN as usize),
        (4 * c, 1),
        (c / 2, 3 * CLSIZB as usize + 7),
    ];
    for &(off, len) in probes {
        assert_eq!(fseek(off, h, 0), off as i32, "Fseek({off}, SEEK_SET)");
        let mut buf = vec![0u8; len];
        let n = fread(h, &mut buf) as usize;
        let want = &all[off as usize..(off as usize + len).min(all.len())];
        assert_eq!(n, want.len(), "Fread({len}) at {off}");
        assert_eq!(&buf[..n], want, "bytes at {off}+{len}");
    }
    // SEEK_CUR / SEEK_END and the range checks.
    assert_eq!(fseek(c, h, 0), c as i32);
    assert_eq!(fseek(-10, h, 1), c as i32 - 10);
    let mut b = [0u8; 20];
    assert_eq!(fread(h, &mut b), 20);
    assert_eq!(&b[..], &all[c as usize - 10..c as usize + 10]);
    assert_eq!(fseek(0, h, 2), FRAG_LEN as i32);
    assert_eq!(fseek(-1, h, 0), -64, "ERANGE");
    assert_eq!(fseek(FRAG_LEN as i64 + 1, h, 0), -64, "ERANGE");
    assert_eq!(fseek(0, h, 3), -32, "EINVFN");
    assert_eq!(fclose(h), 0);
}

#[test]
fn mixed_partial_and_whole_record_reads_across_boundaries() {
    let (_g, _host) = fs();
    let all = expected(SEQ_SEED, SEQ_LEN);
    let h = fopen("C:\\SEQ.DAT", 0);
    assert!(h >= 6);
    let mut pos = 0usize;
    for &len in [1usize, 511, 512, 3, 2045, 2048, 4096, 7, 8192 + 513, 100, 30000, 1, 99999].iter() {
        let mut buf = vec![0u8; len];
        let n = fread(h, &mut buf) as usize;
        let end = (pos + len).min(all.len());
        assert_eq!(n, end - pos, "Fread({len}) at {pos}");
        assert_eq!(&buf[..n], &all[pos..end], "bytes at {pos}+{len}");
        pos = end;
    }
    assert_eq!(pos, all.len());
    assert_eq!(fclose(h), 0);
}

#[test]
fn write_then_read_back_and_both_fats_agree() {
    let (_g, host) = fs();
    let _ = fdelete("C:\\NEW.DAT");
    let h = fcreate("C:\\NEW.DAT");
    assert!(h >= 6, "Fcreate -> {h}");
    // Mixed chunk sizes: partial, whole records, whole clusters, unaligned.
    let chunks = [100usize, 1000, 4096, 5000, 3, 20000, 512, 2048, 777];
    let total: usize = chunks.iter().sum();
    let data: Vec<u8> = (0..total as u32).map(|i| pattern(0xC3, i)).collect();
    let mut at = 0;
    for &c in &chunks {
        assert_eq!(fwrite(h, &data[at..at + c]), c as i32, "Fwrite({c}) at {at}");
        at += c;
    }
    // Overwrite a stretch in the middle that spans a cluster boundary.
    let patch: Vec<u8> = (0..3000u32).map(|i| (i % 253) as u8).collect();
    assert_eq!(fseek(5000, h, 0), 5000);
    assert_eq!(fwrite(h, &patch), 3000);
    assert_eq!(fseek(0, h, 2), total as i32, "the overwrite does not grow the file");
    assert_eq!(fclose(h), 0);

    let mut want = data.clone();
    want[5000..8000].copy_from_slice(&patch);

    for chunk in [512usize, 333, 65536] {
        let h = fopen("C:\\NEW.DAT", 0);
        assert!(h >= 6);
        assert_eq!(read_all(h, chunk), want, "read back in {chunk}-byte chunks");
        assert_eq!(fclose(h), 0);
    }

    // Both FAT copies are identical on the card (FAT buffers are cached once
    // and written to every copy, EmuTOS `flush`, `bdos/fsbuf.c:95-102`).
    for s in 0..FSIZ {
        assert_eq!(host.raw(fat_lba(0) + s as u64), host.raw(fat_lba(1) + s as u64), "FAT sector {s}");
    }
    assert_eq!(fdelete("C:\\NEW.DAT"), 0);
    for s in 0..FSIZ {
        assert_eq!(host.raw(fat_lba(0) + s as u64), host.raw(fat_lba(1) + s as u64), "FAT sector {s} after delete");
    }
}

#[test]
fn whole_record_writes_go_straight_to_the_device_and_stale_copies_are_dropped() {
    let (_g, host) = fs();
    let _ = fdelete("C:\\COH.DAT");
    let h = fcreate("C:\\COH.DAT");
    assert!(h >= 6);
    let first = vec![0xAAu8; 8192];
    host.clear_log();
    assert_eq!(fwrite(h, &first), 8192);
    let writes: Vec<Req> =
        host.log().into_iter().filter(|r| r.write && is_data(r.lba)).collect();
    assert_eq!(writes.len(), 1, "8 KiB of whole records = one request: {writes:?}");
    assert_eq!(writes[0].sectors, 16);
    assert_eq!(fclose(h), 0);

    // Read a few bytes of record 1 (now cached), overwrite the whole record
    // directly, read the bytes again: the cached copy must not survive the
    // direct write (EmuTOS `usrio` invalidates, `bdos/fsio.c:61-71`).
    let h = fopen("C:\\COH.DAT", 2);
    assert!(h >= 6);
    let mut b = [0u8; 4];
    assert_eq!(fseek(512 + 10, h, 0), 522);
    assert_eq!(fread(h, &mut b), 4);
    assert_eq!(b, [0xAA; 4]);
    assert_eq!(fseek(512, h, 0), 512);
    assert_eq!(fwrite(h, &[0x55u8; 512]), 512);
    assert_eq!(fseek(512 + 10, h, 0), 522);
    assert_eq!(fread(h, &mut b), 4);
    assert_eq!(b, [0x55; 4], "the direct write must invalidate the cached record");

    // A partial write (through the cache) followed by a whole-record read of
    // the same record must see the new bytes.
    assert_eq!(fseek(1024 + 100, h, 0), 1124);
    assert_eq!(fwrite(h, &[0x11u8; 8]), 8);
    assert_eq!(fseek(1024, h, 0), 1024);
    let mut rec = [0u8; 512];
    assert_eq!(fread(h, &mut rec), 512);
    assert_eq!(&rec[100..108], &[0x11; 8]);
    assert_eq!(rec[99], 0xAA);
    assert_eq!(rec[108], 0xAA);
    assert_eq!(fclose(h), 0);
    assert_eq!(fdelete("C:\\COH.DAT"), 0);
}
