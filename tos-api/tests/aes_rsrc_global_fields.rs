//! A35 — `rsrc_load` reports the loaded resource back through the caller's
//! `AESGLOBAL`: `ap_ptree`, `ap_rscmem` and `ap_rsclen`.
//!
//! EmuTOS `rs_readit` (`aes/gemrslib.c:785-788`) writes two of them as soon as
//! the file is in memory, and `fix_trindex` (`:597`) the third:
//!
//! ```c
//! rs_global = pglobal;
//! rs_global->ap_rscmem = rs_hdr;          /* gemrslib.c:787 */
//! rs_global->ap_rsclen = rslsize;         /* gemrslib.c:788 */
//! ...
//! ptreebase = (LONG *)get_sub(0, rs_hdr->rsh_trindex, sizeof(LONG));
//! rs_global->ap_ptree = (OBJECT **)ptreebase;   /* gemrslib.c:597 */
//! ```
//!
//! `rs_sglobe` (`gemrslib.c:675-679`) reads them back out of the caller's
//! `AESGLOBAL` on every `rs_free`/`rs_gaddr`/`rs_saddr`, so in EmuTOS these
//! three words *are* the resource descriptor. Applications read them too:
//! TeraDesk uses `_AESrscfile` (`global[5]`) and `_AESrscmem` (`global[7]`) in
//! `icon.c:2692,2707-2708` and sets them itself in `rsc_load.c:417-423`.
//!
//! Audit finding B3, `docs/analysis/audit-applinit-rsrc-load-2026-09-11.md:168-190`:
//! NewTOS's `rsrc_load` never touches `pb.global_ptr()`, so all three read 0
//! after a successful load.
//!
//! Spec: `docs/api/aes/application.md` §`AESGLOBAL`,
//! `docs/api/aes/resource-fileselector.md` §1 `rsrc_load (110)`.
//!
//! **Why these tests are still `#[ignore]`d.** The first blocker — which
//! `AESGLOBAL` layout NewTOS commits to — was decided on 2026-09-13 and is
//! gone: the struct keeps the TOS widths on every ABI (30 bytes, 4-byte
//! pointer fields; `tos-api/src/aes/appl.rs`, `docs/api/aes/application.md`
//! §`AESGLOBAL`), and gemlib reads them at those widths through
//! `sdk/gemlib/overlay/{gem.h,mt_gem.h.in}`. So every field now has its own
//! bytes, and `ap_rsclen` is writable today.
//!
//! Two questions the layout does **not** answer keep the three together:
//!
//! 1. **`ap_ptree` has no array to point at.** EmuTOS publishes the
//!    *relocated* `OBJECT *[]` tree index inside the blob (`fix_trindex`,
//!    `gemrslib.c:595-597`). NewTOS's NRSC tree index is `[u32]` of root
//!    OBJECT **indices** (`tools/mkrsc.py:417`), not addresses — and four
//!    bytes wide in both ABI variants, while `_AESrscfile[i]` is a native
//!    8-byte `OBJECT *` on LP64. Pointing `ap_ptree` at it would hand the
//!    application an index array typed as a pointer array, which is worse
//!    than the 0 it reads today. A faithful `ap_ptree` needs a materialised
//!    caller-width pointer array somewhere the application can reach.
//! 2. **The kernel path has no 32-bit address to report.** `ap_ptree` and
//!    `ap_rscmem` are 4-byte TOS fields. An EL0 caller's blob is mapped in
//!    the process window below `USER_TPA_MAX_VA` (32 MiB), so a user VA fits
//!    exactly. These tests drive the in-kernel path, where the blob is
//!    `RSC_BUF` — a kernel high-half static (measured on the host harness:
//!    `0x1_0444_ff48`; on the target `0xFFFF_0000_0008_xxxx`). Its low 32
//!    bits are not an address anything can use.
//!
//! Writing two of the three and leaving `ap_rscmem` at 0 is not an option
//! either: EmuTOS reads `ap_rscmem == NULL` as "nothing loaded"
//! (`rs_free`, `gemrslib.c:685`), so a non-zero `ap_rsclen` beside a zero
//! `ap_rscmem` states something untrue. They land together or not at all.

use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::bdos::process::Abi;
use tos_api::host::Host;

/// The AES's own file-selector resource — a genuine `NRSC` blob, so a passing
/// test proves the *load* and not just the lookup.
const RSC: &[u8] = include_bytes!("../src/aes/gem_fsel.nrsc");

// ── FAT16 volume, built in host memory ────────────────────────────────────
//
// Same geometry as `tests/fatvol`, `tests/aes_rsrc_load_path.rs` and
// `tests/aes_rsrc_short_blob.rs`; this binary needs only one file, so it
// builds the smallest image that carries it.

const SECTOR: usize = 512;
const PART_LBA: u32 = 64;
const CLSIZ: u32 = 1;
const RESERVED: u32 = 1;
const NUM_FATS: u32 = 2;
const ROOT_ENTRIES: u32 = 512;
const RDLEN: u32 = ROOT_ENTRIES * 32 / SECTOR as u32;
/// Must exceed `MAX_FAT12_CLUSTERS` (4084, `references/emutos/bios/blkdev.h:25`)
/// or the BPB parser calls the volume FAT12.
const NUMCL: u32 = 5000;
const FSIZ: u32 = ((NUMCL + 2) * 2).div_ceil(SECTOR as u32);
const FAT1: u32 = RESERVED;
const ROOT_SEC: u32 = RESERVED + NUM_FATS * FSIZ;
const DATA_SEC: u32 = ROOT_SEC + RDLEN;
const PART_SECTORS: u32 = DATA_SEC + NUMCL * CLSIZ;

fn dirent(name: &[u8; 11], attr: u8, cluster: u16, size: u32) -> [u8; 32] {
    let mut e = [0u8; 32];
    e[0..11].copy_from_slice(name);
    e[0x0B] = attr;
    e[0x16..0x18].copy_from_slice(&0x8000u16.to_le_bytes());
    e[0x18..0x1A].copy_from_slice(&0x5901u16.to_le_bytes());
    e[0x1A..0x1C].copy_from_slice(&cluster.to_le_bytes());
    e[0x1C..0x20].copy_from_slice(&size.to_le_bytes());
    e
}

/// `C:\TEST.RSC` — the embedded resource, in the root of one FAT16 partition.
fn build_image() -> Vec<u8> {
    let total = (PART_LBA + PART_SECTORS) as usize;
    let mut img = vec![0u8; total * SECTOR];

    let e = 0x1BE;
    img[e + 4] = 0x06; // FAT16 partition type
    img[e + 8..e + 12].copy_from_slice(&PART_LBA.to_le_bytes());
    img[e + 12..e + 16].copy_from_slice(&PART_SECTORS.to_le_bytes());
    img[510] = 0x55;
    img[511] = 0xAA;

    let part = PART_LBA as usize * SECTOR;
    {
        let b = &mut img[part..part + SECTOR];
        b[0x0B..0x0D].copy_from_slice(&(SECTOR as u16).to_le_bytes());
        b[0x0D] = CLSIZ as u8;
        b[0x0E..0x10].copy_from_slice(&(RESERVED as u16).to_le_bytes());
        b[0x10] = NUM_FATS as u8;
        b[0x11..0x13].copy_from_slice(&(ROOT_ENTRIES as u16).to_le_bytes());
        b[0x16..0x18].copy_from_slice(&(FSIZ as u16).to_le_bytes());
        b[0x20..0x24].copy_from_slice(&PART_SECTORS.to_le_bytes());
        b[510] = 0x55;
        b[511] = 0xAA;
    }

    // One chain of `n` clusters starting at cluster 2 holds the whole file.
    let n = RSC.len().div_ceil(CLSIZ as usize * SECTOR).max(1);
    for copy in 0..NUM_FATS {
        let fat = part + ((FAT1 + copy * FSIZ) as usize) * SECTOR;
        img[fat..fat + 2].copy_from_slice(&0xFFF8u16.to_le_bytes());
        img[fat + 2..fat + 4].copy_from_slice(&0xFFFFu16.to_le_bytes());
        for i in 0..n {
            let cl = 2 + i;
            let next: u16 = if i + 1 == n { 0xFFFF } else { cl as u16 + 1 };
            let o = fat + cl * 2;
            img[o..o + 2].copy_from_slice(&next.to_le_bytes());
        }
    }

    let root = part + ROOT_SEC as usize * SECTOR;
    img[root..root + 32].copy_from_slice(&dirent(b"TEST    RSC", 0x20, 2, RSC.len() as u32));

    let mut done = 0usize;
    for i in 0..n {
        let off = part + (DATA_SEC as usize + i * CLSIZ as usize) * SECTOR;
        let take = (CLSIZ as usize * SECTOR).min(RSC.len() - done);
        img[off..off + take].copy_from_slice(&RSC[done..done + take]);
        done += take;
    }

    img
}

// ── block device + host ───────────────────────────────────────────────────

struct RamDisk {
    img: Mutex<Vec<u8>>,
}

impl tos_api::bios::block::BlockDev for RamDisk {
    fn block_count(&self) -> u64 {
        (self.img.lock().unwrap().len() / SECTOR) as u64
    }
    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> bool {
        let img = self.img.lock().unwrap();
        let n = buf.len() / SECTOR;
        let off = lba as usize * SECTOR;
        if off + n * SECTOR > img.len() {
            return false;
        }
        buf.copy_from_slice(&img[off..off + n * SECTOR]);
        true
    }
    fn write_blocks(&self, lba: u64, buf: &[u8]) -> bool {
        let mut img = self.img.lock().unwrap();
        let n = buf.len() / SECTOR;
        let off = lba as usize * SECTOR;
        if off + n * SECTOR > img.len() {
            return false;
        }
        img[off..off + n * SECTOR].copy_from_slice(buf);
        true
    }
}

struct DiskHost {
    disk: RamDisk,
}

impl Host for DiskHost {
    fn console_write_byte(&self, _byte: u8) {}
    fn block(&self) -> Option<&dyn tos_api::bios::block::BlockDev> {
        Some(&self.disk)
    }
}

static HOST: OnceLock<()> = OnceLock::new();
/// The BDOS file system and the AES resource descriptor are `UnsafeCell`
/// statics with no locking (single-CPU, IRQ-off trap path), so the tests in
/// this binary must not overlap.
static FS_LOCK: Mutex<()> = Mutex::new(());

fn fs<'a>() -> MutexGuard<'a, ()> {
    let guard = FS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    HOST.get_or_init(|| {
        let host: &'static DiskHost = Box::leak(Box::new(DiskHost {
            disk: RamDisk {
                img: Mutex::new(build_image()),
            },
        }));
        let slot: &'static &'static dyn Host = Box::leak(Box::new(host as &'static dyn Host));
        tos_api::host::install_host(slot);
        assert_eq!(tos_api::bios::mount(), 1, "one FAT16 partition expected");
    });
    guard
}

// ── AES call helpers ──────────────────────────────────────────────────────

fn cstr(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

/// `rsrc_load(name)` through the real dispatcher, handing it a **caller**
/// `AESGLOBAL` of the 16 WORDs gemlib declares (`mt_gem.h:2502`,
/// `AES_GLOBMAX 16`) and returning it together with the result.
fn rsrc_load_with_global(name: &str) -> (i16, [i16; 16]) {
    let spec = cstr(name);
    let mut control = [110i16, 0, 1, 1, 0];
    let mut global = [0i16; 16];
    let mut intin = [0i16; 4];
    let mut intout = [0i16; 8];
    let mut addrin = [spec.as_ptr() as u64, 0];
    let mut addrout = [0u64; 2];
    let raw = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];
    // SAFETY: a well-formed LP64 parameter block, alive for the call.
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
    (intout[0], global)
}

/// `rsrc_gaddr(rtype, index)` → (return code, address).
fn gaddr(rtype: i16, index: i16) -> (i16, u64) {
    let mut control = [112i16, 2, 1, 0, 0];
    let mut global = [0i16; 16];
    let mut intin = [rtype, index, 0, 0];
    let mut intout = [0i16; 8];
    let mut addrin = [0u64; 2];
    let mut addrout = [0u64; 2];
    let raw = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];
    // SAFETY: a well-formed LP64 parameter block, alive for the call.
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
    (intout[0], addrout[0])
}

fn rsrc_free() {
    let mut control = [111i16, 0, 1, 0, 0];
    let mut global = [0i16; 16];
    let mut intin = [0i16; 4];
    let mut intout = [0i16; 8];
    let mut addrin = [0u64; 2];
    let mut addrout = [0u64; 2];
    let raw = [
        control.as_mut_ptr() as u64,
        global.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        addrin.as_mut_ptr() as u64,
        addrout.as_mut_ptr() as u64,
    ];
    // SAFETY: a well-formed LP64 parameter block, alive for the call.
    unsafe { tos_api::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
}

/// The 4-byte pointer field at byte `off` of the caller's `AESGLOBAL` — the
/// TOS width, and the width the NewTOS gemlib overlay reads `_AESrscfile`
/// (byte 10) and `_AESrscmem` (byte 14) at.
fn g_ptr(global: &[i16; 16], off: usize) -> u64 {
    let mut v = [0u8; 4];
    // SAFETY: `global` is 32 bytes; `off + 4 <= 32` for every caller below.
    let bytes = unsafe { std::slice::from_raw_parts(global.as_ptr() as *const u8, 32) };
    v.copy_from_slice(&bytes[off..off + 4]);
    u64::from(u32::from_le_bytes(v))
}

/// `ap_rsclen` — `aes_global[9]`, a WORD in both ABIs.
fn g_rsclen(global: &[i16; 16]) -> u16 {
    global[9] as u16
}

// ── the three fields ──────────────────────────────────────────────────────

/// EmuTOS `gemrslib.c:787` — `rs_global->ap_rscmem = rs_hdr`, the address the
/// resource was loaded at. `rsrc_gaddr(R_TEDINFO, 0)` names a record inside
/// that same blob, so the blob base must be at or below it.
#[test]
#[ignore = "blocked: the in-kernel path has no 32-bit address for the \
            4-byte ap_rscmem field — see the module comment (A35)"]
fn rsrc_load_reports_ap_rscmem() {
    let _g = fs();
    let (ret, global) = rsrc_load_with_global("C:\\TEST.RSC");
    assert_eq!(ret, 1, "rsrc_load must succeed");

    let (ok, ted) = gaddr(2 /* R_TEDINFO */, 0);
    assert_eq!(ok, 1);
    let rscmem = g_ptr(&global, 14);
    assert_ne!(rscmem, 0, "ap_rscmem (global[7]) is still 0 after rsrc_load");
    assert!(
        rscmem <= ted && ted < rscmem + RSC.len() as u64,
        "ap_rscmem {rscmem} does not address the blob that holds tedinfo[0] ({ted})"
    );
    rsrc_free();
}

/// EmuTOS `gemrslib.c:788` — `rs_global->ap_rsclen = rslsize`, the byte length
/// the header declares.
#[test]
#[ignore = "blocked: lands together with ap_ptree/ap_rscmem — see the module comment (A35)"]
fn rsrc_load_reports_ap_rsclen() {
    let _g = fs();
    let (ret, global) = rsrc_load_with_global("C:\\TEST.RSC");
    assert_eq!(ret, 1, "rsrc_load must succeed");
    let total = u32::from_le_bytes(RSC[24..28].try_into().unwrap());
    assert_eq!(
        g_rsclen(&global) as u32,
        total,
        "ap_rsclen (global[9]) must be the resource's declared size"
    );
    rsrc_free();
}

/// EmuTOS `gemrslib.c:595-597` — `fix_trindex` publishes the tree-index array
/// through `ap_ptree`, so `ap_ptree[i]` is tree `i`'s root, the same address
/// `rsrc_gaddr(R_TREE, i)` returns.
#[test]
#[ignore = "blocked: NewTOS's NRSC tree index is an index array, not the \
            relocated pointer array ap_ptree names — see the module comment (A35)"]
fn rsrc_load_reports_ap_ptree() {
    let _g = fs();
    let (ret, global) = rsrc_load_with_global("C:\\TEST.RSC");
    assert_eq!(ret, 1, "rsrc_load must succeed");

    let ptree = g_ptr(&global, 10);
    assert_ne!(ptree, 0, "ap_ptree (global[5]) is still 0 after rsrc_load");
    let (ok, tree0) = gaddr(0 /* R_TREE */, 0);
    assert_eq!(ok, 1);
    // SAFETY: `ap_ptree` addresses the relocated tree-index array.
    let slot0 = unsafe { (ptree as *const u64).read_unaligned() };
    assert_eq!(slot0, tree0, "ap_ptree[0] must be tree 0's root object");
    rsrc_free();
}

/// The state the audit measured, kept as a live statement of what an
/// application sees today: a successful `rsrc_load` leaves all three fields 0.
/// This is the counterpart of the three ignored tests above — when they are
/// enabled, this one goes.
#[test]
fn rsrc_load_leaves_the_three_fields_zero_today() {
    let _g = fs();
    let (ret, global) = rsrc_load_with_global("C:\\TEST.RSC");
    assert_eq!(ret, 1, "rsrc_load must succeed");
    assert_eq!(g_ptr(&global, 10), 0, "ap_ptree");
    assert_eq!(g_ptr(&global, 14), 0, "ap_rscmem");
    assert_eq!(g_rsclen(&global), 0, "ap_rsclen");
    rsrc_free();
}
