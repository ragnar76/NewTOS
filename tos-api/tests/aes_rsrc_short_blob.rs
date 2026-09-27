//! `rsrc_load` (110) must refuse a `.RSC` it could not read whole, instead of
//! relocating a blob past the bytes that exist.
//!
//! EmuTOS `rs_readit` (`aes/gemrslib.c:775-783`) allocates exactly the size the
//! resource declares and insists on reading that many bytes:
//!
//! ```c
//! rs_hdr = (RSHDR *)dos_alloc_anyram(rslsize);
//! if (!rs_hdr)
//!     return FALSE;
//! if (dos_lseek(fd, 0, 0x0L) < 0L)
//!     return FALSE;
//! if (dos_read(fd, rslsize, rs_hdr) != rslsize)
//!     return FALSE;           /* error or short read */
//! ```
//!
//! Only then does it run `fix_trindex`/`fix_objects`/`fix_tedinfo_std`/
//! `fix_nptrs`, which *write* through every declared offset. A file that is
//! short — or too large for the memory the loader got — never reaches them:
//! `rs_readit` returns FALSE and `rsrc_load` reports 0.
//!
//! NewTOS reads into a fixed buffer ([`tos_api::aes::resource::RSC_CAP`]) via
//! `bdos::fs::read_file`, which stops at the buffer's end
//! (`tos-api/src/bdos/fs/mod.rs:142`, `let want = (entry.size as
//! usize).min(out.len())`) and reports only how much it copied. Before this
//! test the loader checked nothing but `len < size_of::<NrscHeader>()`, so a
//! short read went straight into `relocate_blob_core`, whose `ob_spec += tb`
//! walk then wrote past the bytes that were read — audit finding B6,
//! `docs/analysis/audit-applinit-rsrc-load-2026-09-11.md:272-300`, reproduced
//! there as a SIGBUS.
//!
//! Spec: `docs/api/aes/resource-fileselector.md` §1 `rsrc_load (110)`.
//!
//! The file-system tests drive the real AES dispatcher against a FAT16 volume
//! built in host memory, so the whole path — `sh_find` → `bdos::fs::read_file`
//! → relocation — is exercised.

use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::aes::resource::RSC_CAP;
use tos_api::bdos::process::Abi;
use tos_api::host::Host;

/// The kernel's own file-selector resource — a genuine `NRSC` blob, so a
/// passing test proves the *load* and not just the lookup.
const RSC: &[u8] = include_bytes!("../src/aes/gem_fsel.nrsc");

// ── FAT16 volume, built in host memory ────────────────────────────────────
//
// Same geometry as `tests/fatvol` and `tests/aes_rsrc_load_path.rs`; this
// binary needs files of its own sizes (a truncated one, one larger than the
// loader's buffer), so it builds its own image.

const SECTOR: usize = 512;
const PART_LBA: u32 = 64;
const CLSIZ: u32 = 1; // sectors per cluster
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

struct Builder {
    img: Vec<u8>,
    next_cluster: u16,
}

impl Builder {
    fn new() -> Builder {
        let total = (PART_LBA + PART_SECTORS) as usize;
        let mut b = Builder {
            img: vec![0u8; total * SECTOR],
            next_cluster: 2,
        };
        b.write_mbr();
        b.write_bpb();
        b.write_fat_head();
        b
    }

    fn part(&self) -> usize {
        PART_LBA as usize * SECTOR
    }

    fn write_mbr(&mut self) {
        let e = 0x1BE;
        self.img[e + 4] = 0x06; // FAT16 partition type
        self.img[e + 8..e + 12].copy_from_slice(&PART_LBA.to_le_bytes());
        self.img[e + 12..e + 16].copy_from_slice(&PART_SECTORS.to_le_bytes());
        self.img[510] = 0x55;
        self.img[511] = 0xAA;
    }

    fn write_bpb(&mut self) {
        let p = self.part();
        let b = &mut self.img[p..p + SECTOR];
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

    fn write_fat_head(&mut self) {
        for copy in 0..NUM_FATS {
            let fat = self.part() + ((FAT1 + copy * FSIZ) as usize) * SECTOR;
            self.img[fat..fat + 2].copy_from_slice(&0xFFF8u16.to_le_bytes());
            self.img[fat + 2..fat + 4].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }
    }

    fn fat_set(&mut self, cluster: u16, value: u16) {
        for copy in 0..NUM_FATS {
            let fat = self.part() + ((FAT1 + copy * FSIZ) as usize) * SECTOR;
            let o = fat + cluster as usize * 2;
            self.img[o..o + 2].copy_from_slice(&value.to_le_bytes());
        }
    }

    fn cluster_off(&self, cluster: u16) -> usize {
        self.part() + (DATA_SEC as usize + (cluster as usize - 2) * CLSIZ as usize) * SECTOR
    }

    /// Allocate `n` linked clusters and return the first.
    fn alloc_chain(&mut self, n: usize) -> u16 {
        assert!(n > 0);
        let first = self.next_cluster;
        for i in 0..n {
            let cl = first + i as u16;
            let next = if i + 1 == n { 0xFFFF } else { cl + 1 };
            self.fat_set(cl, next);
            let off = self.cluster_off(cl);
            self.img[off..off + CLSIZ as usize * SECTOR].fill(0);
        }
        self.next_cluster += n as u16;
        first
    }

    /// Add a 32-byte entry to the root directory.
    fn dir_add(&mut self, entry: [u8; 32]) {
        let base = self.part() + ROOT_SEC as usize * SECTOR;
        let span = RDLEN as usize * SECTOR;
        for slot in (0..span).step_by(32) {
            if self.img[base + slot] == 0 {
                self.img[base + slot..base + slot + 32].copy_from_slice(&entry);
                return;
            }
        }
        panic!("root directory full");
    }

    /// Write `data` as a root-directory file, the directory entry carrying its
    /// true length (so `read_file` sees the size the card would report).
    fn mkfile(&mut self, name: &str, data: &[u8]) {
        let per = CLSIZ as usize * SECTOR;
        let n = data.len().div_ceil(per).max(1);
        let first = self.alloc_chain(n);
        let mut done = 0usize;
        for i in 0..n {
            let off = self.cluster_off(first + i as u16);
            let take = per.min(data.len() - done);
            self.img[off..off + take].copy_from_slice(&data[done..done + take]);
            done += take;
        }
        self.dir_add(dirent(&fcb(name), 0x20, first, data.len() as u32));
    }
}

/// `"WHOLE.RSC"` → the eleven-byte 8.3 FCB name.
fn fcb(name: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    let (stem, ext) = match name.split_once('.') {
        Some((s, e)) => (s, e),
        None => (name, ""),
    };
    assert!(stem.len() <= 8 && ext.len() <= 3, "not an 8.3 name: {name}");
    for (i, c) in stem.bytes().enumerate() {
        out[i] = c.to_ascii_uppercase();
    }
    for (i, c) in ext.bytes().enumerate() {
        out[8 + i] = c.to_ascii_uppercase();
    }
    out
}

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

/// Byte offset of `total_size` in the `NRSC` header (magic + six `u32`),
/// which sits at the front (NRSC v1–v12) or behind the 40-byte native
/// `RSHDR` preamble (v13+, `resource.rs` `RSHDR_PAD`).
fn total_size_off() -> usize {
    let off = if &RSC[..4] == b"NRSC" { 0 } else { 40 };
    assert_eq!(&RSC[off..off + 4], b"NRSC", "an NRSC blob");
    off + 24
}
/// Byte offset of `rsh_rssize` in the native `RSHDR` preamble (field 17).
const RSH_RSSIZE_OFF: usize = 34;

/// The same resource padded out past `RSC_CAP`, its `total_size` telling the
/// truth about the padded length: a well-formed file that simply does not fit
/// the loader's buffer. This is TeraDesk's `cicons.rsc` case — 63 788 bytes
/// classic, larger once converted.
fn oversized() -> Vec<u8> {
    let mut v = RSC.to_vec();
    v.resize(RSC_CAP + 4096, 0);
    let total = v.len() as u32;
    let off = total_size_off();
    v[off..off + 4].copy_from_slice(&total.to_le_bytes());
    if off != 24 {
        // A v13+ image: its `RSHDR` preamble states the size too (`rsh_rssize`,
        // a u16 — the low word, exactly what EmuTOS's `ap_rsclen` would carry).
        v[RSH_RSSIZE_OFF..RSH_RSSIZE_OFF + 2].copy_from_slice(&(total as u16).to_le_bytes());
    }
    v
}

/// ```text
/// C:\WHOLE.RSC    the resource, complete
/// C:\HALF.RSC     the first half of it, its header still declaring the whole
/// C:\HUGE.RSC     a well-formed resource larger than the loader's buffer
/// ```
fn build_image() -> Vec<u8> {
    let mut b = Builder::new();
    b.mkfile("WHOLE.RSC", RSC);
    b.mkfile("HALF.RSC", &RSC[..RSC.len() / 2]);
    b.mkfile("HUGE.RSC", &oversized());
    b.img
}

// ── block device + host ───────────────────────────────────────────────────

struct RamDisk {
    img: Mutex<Vec<u8>>,
}

impl tos_api::bios::block::BlockDev for RamDisk {
    fn block_count(&self) -> u64 {
        (self.img.lock().unwrap().len() / SECTOR) as u64
    }
    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<(), i32> {
        let img = self.img.lock().unwrap();
        let n = buf.len() / SECTOR;
        let off = lba as usize * SECTOR;
        if off + n * SECTOR > img.len() {
            return Err(tos_types::error::ESECNF);
        }
        buf.copy_from_slice(&img[off..off + n * SECTOR]);
        Ok(())
    }
    fn write_blocks(&self, lba: u64, buf: &[u8]) -> Result<(), i32> {
        let mut img = self.img.lock().unwrap();
        let n = buf.len() / SECTOR;
        let off = lba as usize * SECTOR;
        if off + n * SECTOR > img.len() {
            return Err(tos_types::error::ESECNF);
        }
        img[off..off + n * SECTOR].copy_from_slice(buf);
        Ok(())
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

static HOST: OnceLock<&'static DiskHost> = OnceLock::new();
/// The BDOS file system and the AES resource descriptor are `UnsafeCell`
/// statics with no locking (single-CPU, IRQ-off trap path), so the tests in
/// this binary must not overlap.
static LOCK: Mutex<()> = Mutex::new(());

fn fs<'a>() -> MutexGuard<'a, ()> {
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    HOST.get_or_init(|| {
        let host: &'static DiskHost = Box::leak(Box::new(DiskHost {
            disk: RamDisk {
                img: Mutex::new(build_image()),
            },
        }));
        let slot: &'static &'static dyn Host = Box::leak(Box::new(host as &'static dyn Host));
        tos_api::host::install_host(slot);
        assert_eq!(tos_api::bios::mount(), 1, "one FAT16 partition expected");
        host
    });
    tos_api::aes::resource::set_app_dir(b"NONE\0");
    guard
}

// ── AES call helpers ──────────────────────────────────────────────────────

fn cstr(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

/// `rsrc_load(name)` through the real dispatcher; returns `int_out[0]`.
fn rsrc_load(name: &str) -> i16 {
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
    intout[0]
}

/// `rsrc_free()` — so one test's loaded resource is not another's.
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

// ── the loader ────────────────────────────────────────────────────────────

/// The whole file still loads — the guard against a length check so strict
/// that it locks the real applications (QED, TeraDesk) out of their resources.
#[test]
fn the_complete_resource_still_loads() {
    let _g = fs();
    assert_eq!(
        rsrc_load("WHOLE.RSC"),
        1,
        "a resource whose bytes are all there must load"
    );
    rsrc_free();
}

/// `dos_read(fd, rslsize, rs_hdr) != rslsize` → FALSE (`gemrslib.c:782-783`).
/// The file holds half the bytes its header declares, so the fix-up walk would
/// run off the end of what was read.
#[test]
fn a_file_shorter_than_its_declared_size_is_refused() {
    let _g = fs();
    assert_eq!(
        rsrc_load("HALF.RSC"),
        0,
        "a short read must fail the load (gemrslib.c:782-783), not be relocated"
    );
    rsrc_free();
}

/// EmuTOS refuses a resource it cannot hold: `dos_alloc_anyram(rslsize)`
/// returning NULL is `return FALSE` (`gemrslib.c:775-777`). NewTOS's buffer is
/// a fixed [`RSC_CAP`], and `read_file` stops at its end without saying so, so
/// the same honest refusal has to come from the declared-size check.
#[test]
fn a_resource_larger_than_the_loader_buffer_is_refused() {
    let _g = fs();
    assert_eq!(
        rsrc_load("HUGE.RSC"),
        0,
        "a resource past RSC_CAP must be refused, not silently truncated \
         and relocated"
    );
    rsrc_free();
}

// ── the relocation itself: no write outside the bytes that exist ───────────
//
// The file-system tests above show the *verdict* (`rsrc_load` = 0). These show
// the memory property behind it: the loader's buffer is 64 KiB, so a blob
// truncated to a few kilobytes still has room to be relocated *inside* it —
// the damage only shows when the blob is as large as the memory it was given.
// That is the audit's reproduction (a `Vec` sized to the truncated blob, which
// ended in SIGBUS), made deterministic here with a guard area behind the blob.

/// Bytes of untouched filler placed behind a blob. Larger than the distance
/// from the truncated blob's end to the furthest offset its header declares
/// (the sub-structs at the end of the full 4 819-byte resource), so a
/// regression lands *in* the guard and is reported, instead of depending on
/// where the allocator happens to put an unmapped page.
const GUARD: usize = 8 * 1024;
/// Filler byte pattern.
const FILL: u8 = 0xA5;

/// A blob of `src` followed by [`GUARD`] filler bytes. Returns the owning
/// allocation (u64 so the OBJECT array is naturally aligned), the blob
/// pointer, and the blob's length.
fn guarded(src: &[u8]) -> (Vec<u64>, *mut u8, usize) {
    let mut v = vec![0u64; (src.len() + GUARD).div_ceil(8)];
    let p = v.as_mut_ptr() as *mut u8;
    // SAFETY: the allocation covers `src.len() + GUARD` bytes.
    unsafe {
        core::ptr::copy_nonoverlapping(src.as_ptr(), p, src.len());
        core::ptr::write_bytes(p.add(src.len()), FILL, GUARD);
    }
    (v, p, src.len())
}

/// The first and last guard byte behind `blob[..len]` that is no longer
/// filler, as offsets past the end of the blob — `None` when the guard is
/// untouched. Reporting the span makes a regression say how far it reached.
fn guard_breach(p: *const u8, len: usize) -> Option<(usize, usize)> {
    // SAFETY: `guarded` placed GUARD filler bytes at `p + len`.
    let guard = unsafe { core::slice::from_raw_parts(p.add(len), GUARD) };
    let first = guard.iter().position(|&b| b != FILL)?;
    let last = guard.iter().rposition(|&b| b != FILL)?;
    Some((first, last))
}

/// Assert nothing was written behind `blob[..len]`.
fn assert_guard_intact(p: *const u8, len: usize) {
    if let Some((first, last)) = guard_breach(p, len) {
        panic!("the relocation wrote past the end of the blob, at +{first}..=+{last}");
    }
}

/// The audit's case (`docs/analysis/audit-applinit-rsrc-load-2026-09-11.md`
/// B6): half a resource, its header still declaring the whole. EmuTOS never
/// gets here — the short `dos_read` fails the load (`gemrslib.c:782-783`).
/// Refused, and — the point of the finding — refused *before* a single byte is
/// written: neither past the blob nor inside it.
#[test]
fn a_truncated_blob_is_refused_without_writing_anywhere() {
    let _g = fs();
    let cut = &RSC[..RSC.len() / 2];
    let (_owner, p, len) = guarded(cut);
    let accepted = unsafe { tos_api::aes::resource::relocate_and_register(p, len) };
    // The memory property first — it is the finding; the verdict follows.
    assert_guard_intact(p, len);
    // SAFETY: `p` addresses the `len`-byte blob.
    let after = unsafe { core::slice::from_raw_parts(p, len) };
    assert_eq!(after, cut, "the refused blob was relocated anyway");
    assert!(
        !accepted,
        "a blob shorter than its declared total_size was accepted"
    );
    rsrc_free();
}

/// The counter-example that keeps the check honest: the same resource, whole,
/// in the same tight buffer. It must load, and it must not write past itself
/// either — a relocated `ob_spec` is a pointer into the blob.
#[test]
fn the_whole_blob_relocates_inside_itself() {
    let _g = fs();
    let (_owner, p, len) = guarded(RSC);
    assert!(
        unsafe { tos_api::aes::resource::relocate_and_register(p, len) },
        "the complete resource must still relocate"
    );
    assert_guard_intact(p, len);
    // SAFETY: `p` addresses the relocated blob; tree 0's root is object 0.
    let root = unsafe { tos_api::aes::resource::tree_root(p, 0) };
    assert!(
        !root.is_null() && (root as usize) >= p as usize,
        "tree 0's root must point into the blob"
    );
    rsrc_free();
}

/// The same hazard reached from the other side: a header whose OBJECT array
/// ends beyond the blob. That is what a resource larger than the loader's
/// buffer looks like once `read_file` has cut it — the arrays are simply not
/// all there — and the fix-up walk would have written at every one of those
/// offsets.
#[test]
fn a_header_whose_arrays_run_past_the_blob_is_refused() {
    let _g = fs();
    const LEN: usize = 512;
    let mut src = vec![0u8; LEN];
    src[0..4].copy_from_slice(b"NRSC");
    let put = |s: &mut Vec<u8>, off: usize, v: u32| {
        s[off..off + 4].copy_from_slice(&v.to_le_bytes());
    };
    put(&mut src, 4, 1); // version 1 (28-byte header generation)
    put(&mut src, 8, 1); // n_trees
    put(&mut src, 12, 4); // n_objects
    put(&mut src, 16, 68); // off_trindex
    put(&mut src, 20, 400); // off_objects — 400 + 4 * 32 = 528 > 512
    put(&mut src, 24, LEN as u32); // total_size of a v1 header at the front
    let (_owner, p, len) = guarded(&src);
    assert!(
        !unsafe { tos_api::aes::resource::relocate_and_register(p, len) },
        "an OBJECT array declared past the end of the blob was accepted"
    );
    assert_guard_intact(p, len);
    rsrc_free();
}
