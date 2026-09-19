//! A35 — `rsrc_load` reports the loaded resource back through the caller's
//! `AESGLOBAL`: `ap_ptree`, `ap_rscmem` and `ap_rsclen`.
//!
//! EmuTOS `rs_readit` (`aes/gemrslib.c:785-788`) writes two of them as soon as
//! the file is in memory, and `fix_trindex` (`:595-597`) the third:
//!
//! ```c
//! rs_global = pglobal;
//! rs_global->ap_rscmem = rs_hdr;          /* gemrslib.c:787 */
//! rs_global->ap_rsclen = rslsize;         /* gemrslib.c:788 */
//! ...
//! ptreebase = (LONG *)get_sub(0, rs_hdr->rsh_trindex, sizeof(LONG));
//! rs_global->ap_ptree = (OBJECT **)ptreebase;   /* gemrslib.c:597 */
//! for (ii = 0; ii < rs_hdr->rsh_ntree; ii++)
//!     fix_long(ptreebase+ii);
//! ```
//!
//! `get_addr` reads that array straight back — `case R_TREE: return
//! rs_global->ap_ptree[rsindex];` (`gemrslib.c:124`) — so `ap_ptree[i]` and
//! `rsrc_gaddr(R_TREE, i)` are the *same value*, and that binding is the point
//! of the whole field. Applications use it: TeraDesk reads `_AESrscfile`
//! (`global[5]`) and `_AESrscmem` (`global[7]`) in `icon.c:2692,2707-2708`.
//!
//! Audit finding B3, `docs/analysis/audit-applinit-rsrc-load-2026-09-11.md:168-190`.
//!
//! Spec: `docs/api/aes/application.md` §`AESGLOBAL`,
//! `docs/api/aes/resource-fileselector.md` §1 `rsrc_load (110)`.
//!
//! ## What this binary can and cannot see (read before adding a test here)
//!
//! It drives the **in-place** load path: `rsrc_load` finds no running process,
//! so it relocates inside `RSC_BUF`, a `tos-api` static. That is the path the
//! kernel would take for an EL1 caller — and the two address fields are four
//! bytes wide on every ABI (`gemrslib.h:15-27`; the decision of 2026-09-13,
//! `docs/api/aes/application.md` §`AESGLOBAL`), so they can only carry a blob
//! that lives below 4 GiB in the caller's address space.
//!
//! On a 64-bit host **no** address is ever below 4 GiB — macOS reserves the
//! first 4 GiB as `__PAGEZERO`, and `nm` on this binary puts `RSC_BUF` at
//! `0x1_0010_7ef8` — so this harness can only ever observe the **refusal**:
//! `rsrc_load` succeeds, `rsrc_gaddr` answers at full width, and the three
//! fields stay 0 rather than carry a truncated address that would name
//! nothing. The tests below pin exactly that, and pin that the value written
//! is not the truncation.
//!
//! The **positive** branch — an EL0 blob in the per-process window below
//! `USER_TPA_MAX_VA` (32 MiB), where all three fields are written — is driven
//! by the in-crate unit tests in `tos-api/src/aes/resource.rs` (`mod tests`),
//! which can hand `relocate_and_register_core` the access/target split
//! `load_into_user` uses. On the machine it is TeraDesk and QED that exercise
//! it: measured on QEMU `raspi4b`, TeraDesk's 68 rsrc calls all carry blob
//! addresses `0x4000ac … 0x409381`.
//!
//! There is no EL1 caller of `rsrc_load` on a running system (measured
//! 2026-09-13; `tos-api/src/aes/resource.rs` module comment), so the refusal
//! branch binds this harness and nothing else.

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

/// The address the blob was relocated onto — EmuTOS's `rs_hdr`, i.e. the value
/// `ap_rscmem` names. Derived from the AES itself rather than from a static:
/// `rsrc_gaddr(R_OBJECT, 0)` is the OBJECT array's base, which the header
/// places `off_objects` bytes into the blob.
fn blob_base() -> u64 {
    let (ok, obj0) = gaddr(1 /* R_OBJECT */, 0);
    assert_eq!(ok, 1, "the resource must be loaded");
    obj0 - u64::from(u32::from_le_bytes(RSC[20..24].try_into().unwrap()))
}

// ── the three fields ──────────────────────────────────────────────────────

/// `fix_trindex` (`gemrslib.c:591-603`) + `get_addr` (`:124`): after a load the
/// blob carries a **relocated** tree index, and slot `i` of it holds exactly the
/// address `rsrc_gaddr(R_TREE, i)` returns. That array is what `ap_ptree`
/// names; NRSC v7/v8 is the generation that has it (`NRSC_VER_TRINDEX_PTR`).
///
/// This is the end-to-end half of A35 the host can see: the blob is relocated
/// in place, so the test dereferences the very array an application would get
/// through `ap_ptree`.
#[test]
fn rsrc_load_publishes_the_relocated_tree_index() {
    let _g = fs();
    let (ret, _global) = rsrc_load_with_global("C:\\TEST.RSC");
    assert_eq!(ret, 1, "rsrc_load must succeed");

    let n_trees = u32::from_le_bytes(RSC[8..12].try_into().unwrap()) as usize;
    assert!(n_trees >= 3, "the AES file-selector resource declares three trees");
    let ptree = blob_base() + u32::from_le_bytes(RSC[16..20].try_into().unwrap()) as u64;
    for i in 0..n_trees {
        // The slot, at the caller's pointer width (LP64 here, 8 bytes).
        // SAFETY: `ptree` addresses the relocated tree index of the blob that
        // is live until `rsrc_free` below.
        let slot = unsafe { (ptree as *const u64).add(i).read_unaligned() };
        let (ok, tree) = gaddr(0 /* R_TREE */, i as i16);
        assert_eq!(ok, 1, "tree {i} must resolve");
        assert_eq!(slot, tree, "ap_ptree[{i}] must be tree {i}'s root object");
    }
    rsrc_free();
}

/// `ap_ptree` / `ap_rscmem` are four-byte fields. When the blob does not fit
/// them — this harness's in-place load into a host static above 4 GiB, the
/// shape of the in-kernel path — they are left at 0, which is EmuTOS's
/// "nothing loaded" (`rs_free`, `gemrslib.c:685`). What they must **never**
/// hold is the low half of the address: that names a page nothing can reach.
#[test]
fn the_two_address_fields_are_never_a_truncated_address() {
    let _g = fs();
    let (ret, global) = rsrc_load_with_global("C:\\TEST.RSC");
    assert_eq!(ret, 1, "rsrc_load must succeed");

    let base = blob_base();
    assert!(
        base > u64::from(u32::MAX),
        "this harness is only meaningful while the blob sits above 4 GiB \
         (measured {base:#x}); see the module comment"
    );
    let truncated = u64::from(base as u32);
    assert_ne!(truncated, 0, "the truncation must be a distinguishable value");

    let ptree_field = g_ptr(&global, 10);
    let rscmem_field = g_ptr(&global, 14);
    assert_eq!(rscmem_field, 0, "ap_rscmem must stay 0, not carry half an address");
    assert_ne!(rscmem_field, truncated, "ap_rscmem must not be the truncation");
    assert_eq!(ptree_field, 0, "ap_ptree must stay 0");
    let tri = u64::from(u32::from_le_bytes(RSC[16..20].try_into().unwrap()));
    assert_ne!(ptree_field, truncated + tri, "ap_ptree must not be the truncation");
    rsrc_free();
}

/// The three land together. `ap_rsclen` is a plain WORD and always
/// representable, but EmuTOS reads `ap_rscmem == NULL` as "nothing loaded"
/// (`gemrslib.c:685`), so a length beside a null base would state two
/// contradictory things — EmuTOS never produces that state (`rs_readit` writes
/// both in adjacent statements, `:787-788`) and neither does NewTOS.
#[test]
fn ap_rsclen_lands_together_with_the_two_address_fields() {
    let _g = fs();
    let (ret, global) = rsrc_load_with_global("C:\\TEST.RSC");
    assert_eq!(ret, 1, "rsrc_load must succeed");
    let total = u32::from_le_bytes(RSC[24..28].try_into().unwrap());
    assert_ne!(total, 0, "the resource declares a size");
    assert_eq!(g_ptr(&global, 14), 0, "precondition: ap_rscmem was not written");
    assert_eq!(
        g_rsclen(&global),
        0,
        "ap_rsclen must not be written while ap_rscmem says nothing is loaded"
    );
    rsrc_free();
}

/// Refusing to publish must not weaken the load: `rsrc_load` still returns 1,
/// `rsrc_gaddr` still answers at full width from the kernel descriptor, and
/// `rsrc_free` still reports that something was loaded. Since A39 stage 2
/// the calls act on the entry the caller's `ap_rscmem` names (EmuTOS's
/// `rs_sglobe` read-back, `gemrslib.c:675-679`); a load that published
/// nothing carries the name 0 the caller was left with, and the zero words
/// reach the process's latest such load (`tos-api/src/aes/resource.rs`,
/// module comment, "a blob the four bytes cannot name") — which is what
/// keeps this in-place harness reachable at all.
#[test]
fn a_load_that_publishes_nothing_still_works() {
    let _g = fs();
    let (ret, _global) = rsrc_load_with_global("C:\\TEST.RSC");
    assert_eq!(ret, 1, "rsrc_load must succeed");
    let (ok, tree) = gaddr(0 /* R_TREE */, 0);
    assert_eq!(ok, 1);
    assert!(tree > u64::from(u32::MAX), "the full-width address is intact");
    let (ok, ted) = gaddr(2 /* R_TEDINFO */, 0);
    assert_eq!(ok, 1);
    assert!(ted >= blob_base() && ted < blob_base() + RSC.len() as u64);
    rsrc_free();
}
