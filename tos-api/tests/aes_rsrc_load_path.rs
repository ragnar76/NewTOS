//! `rsrc_load` (110) resolves its filename the way EmuTOS `rs_load` does:
//! through `sh_find` (`aes/gemrslib.c:835-837`), i.e. `findfile`'s four stages
//! (`aes/gemshlib.c:406-472`):
//!
//! 1. `rlr->p_appdir` + the **name portion** of the spec (`sh_name`),
//! 2. the spec **unchanged** (current directory — this is what makes any
//!    qualified path, absolute or relative, resolvable),
//! 3. `\<name>` on the **current drive**,
//! 4. the AES `PATH=` (NewTOS has no loader-populated AES environment; see
//!    `aes::shell::sh_find`).
//!
//! Spec: `docs/api/aes/resource-fileselector.md` §1 `rsrc_load (110)` and
//! "Edge cases discovered in source" #1.
//!
//! The regression this guards (audit B1,
//! `docs/analysis/audit-applinit-rsrc-load-2026-09-11.md:37-106`): `rsrc_load`
//! copied the filename into a 16-byte buffer and *always* prefixed one of
//! three fixed directories, so `C:\APPS\QED\qed.rsc` — what QED passes
//! (`sdk/apps/qed/src/rsc.c:50-72`) — became `C:\APPS\QED\qed` and then had a
//! prefix pasted in front of it. Every one of QED's three attempts failed and
//! the user saw "Resource file not found!".
//!
//! Each test drives the real AES dispatcher against a FAT16 volume built in
//! host memory, so the whole path — `sh_find` → `bdos::fs::read_file` →
//! relocation — is exercised, not a fragment of it.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::bdos::process::Abi;
use tos_api::host::Host;
use tos_api::trap::{TrapArgs, TrapClass};

// ── a real native resource ────────────────────────────────────────────────

/// The kernel's own `GEM.RSC` file selector, a genuine `NRSC` blob: using a
/// real resource means a passing test proves the *load*, not just the lookup.
const RSC: &[u8] = include_bytes!("../src/aes/gem_fsel.nrsc");

// ── FAT16 volume, built in host memory ────────────────────────────────────
//
// Same geometry as `tests/fatvol`, but this binary needs its own directory
// tree (nested application directories, a >64-byte path), so it builds its
// own image rather than widening the shared one.

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

/// `0` addresses the fixed root directory; anything else is a data cluster.
const ROOT: u16 = 0;

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

    /// Add a 32-byte directory entry to `parent` ([`ROOT`] = root directory).
    fn dir_add(&mut self, parent: u16, entry: [u8; 32]) {
        let (base, span) = if parent == ROOT {
            (
                self.part() + ROOT_SEC as usize * SECTOR,
                RDLEN as usize * SECTOR,
            )
        } else {
            (
                self.cluster_off(parent),
                CLSIZ as usize * SECTOR, // one cluster per directory here
            )
        };
        for slot in (0..span).step_by(32) {
            if self.img[base + slot] == 0 {
                self.img[base + slot..base + slot + 32].copy_from_slice(&entry);
                return;
            }
        }
        panic!("directory full");
    }

    fn mkdir(&mut self, parent: u16, name: &str) -> u16 {
        let cl = self.alloc_chain(1);
        self.dir_add(parent, dirent(&fcb(name), 0x10, cl, 0));
        self.dir_add(cl, dirent(b".          ", 0x10, cl, 0));
        self.dir_add(cl, dirent(b"..         ", 0x10, parent, 0));
        cl
    }

    fn mkfile(&mut self, parent: u16, name: &str, data: &[u8]) {
        let n = data.len().div_ceil(CLSIZ as usize * SECTOR).max(1);
        let first = self.alloc_chain(n);
        let mut done = 0usize;
        for i in 0..n {
            let off = self.cluster_off(first + i as u16);
            let take = (CLSIZ as usize * SECTOR).min(data.len() - done);
            self.img[off..off + take].copy_from_slice(&data[done..done + take]);
            done += take;
        }
        self.dir_add(parent, dirent(&fcb(name), 0x20, first, data.len() as u32));
    }

    /// Create `C:\A\B\C\...` and return the deepest cluster.
    fn mkdirs(&mut self, parent: u16, names: &[&str]) -> u16 {
        let mut cur = parent;
        for n in names {
            cur = self.mkdir(cur, n);
        }
        cur
    }
}

/// `"QED.RSC"` → the eleven-byte 8.3 FCB name.
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

/// The seven-level directory chain whose path is longer than the 64-byte
/// buffers `rsrc_load` used to compose into (`resource.rs:941`) and than the
/// old `APP_DIR_CAP` (`resource.rs:840`). `C:\` + 7 × `NNNNNNNN\` = 66 bytes
/// of directory, `+ "DEEP.RSC"` = 74 bytes of full path.
const DEEP: [&str; 7] = [
    "AAAAAAAA", "BBBBBBBB", "CCCCCCCC", "DDDDDDDD", "EEEEEEEE", "FFFFFFFF", "GGGGGGGG",
];
const DEEP_DIR: &str = "C:\\AAAAAAAA\\BBBBBBBB\\CCCCCCCC\\DDDDDDDD\\EEEEEEEE\\FFFFFFFF\\GGGGGGGG\\";
const DEEP_PATH: &str = "C:\\AAAAAAAA\\BBBBBBBB\\CCCCCCCC\\DDDDDDDD\\EEEEEEEE\\FFFFFFFF\\GGGGGGGG\\DEEP.RSC";

/// ```text
/// C:\ROOTONLY.RSC                 only at the root of the current drive
/// C:\APPS\QED\QED.RSC             an application directory, like the card
/// C:\APPS\QED\SUB\                a directory below it, for a relative spec
/// C:\AAAAAAAA\…\GGGGGGGG\DEEP.RSC a 74-byte fully-qualified path
/// ```
fn build_image() -> Vec<u8> {
    let mut b = Builder::new();
    b.mkfile(ROOT, "ROOTONLY.RSC", RSC);
    let apps = b.mkdir(ROOT, "APPS");
    let qed = b.mkdir(apps, "QED");
    b.mkfile(qed, "QED.RSC", RSC);
    b.mkdir(qed, "SUB");
    let deep = b.mkdirs(ROOT, &DEEP);
    b.mkfile(deep, "DEEP.RSC", RSC);
    b.img
}

// ── block device + host ───────────────────────────────────────────────────

struct RamDisk {
    img: Mutex<Vec<u8>>,
    reads: AtomicU32,
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
        self.reads.fetch_add(n as u32, Ordering::Relaxed);
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
static FS_LOCK: Mutex<()> = Mutex::new(());

fn fs<'a>() -> MutexGuard<'a, ()> {
    let guard = FS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    HOST.get_or_init(|| {
        let host: &'static DiskHost = Box::leak(Box::new(DiskHost {
            disk: RamDisk {
                img: Mutex::new(build_image()),
                reads: AtomicU32::new(0),
            },
        }));
        let slot: &'static &'static dyn Host = Box::leak(Box::new(host as &'static dyn Host));
        tos_api::host::install_host(slot);
        assert_eq!(tos_api::bios::mount(), 1, "one FAT16 partition expected");
        host
    });
    // Every test starts on C:, at its root, with no application directory —
    // the state each one then varies deliberately.
    assert_ne!(gemdos1(DSETDRV, 2) & 0b100, 0, "C: must be in the drive map");
    assert_eq!(gemdos1(DSETPATH, cstr("\\").as_ptr() as u64), 0);
    clear_app_dir();
    guard
}

// ── GEMDOS / AES call helpers ─────────────────────────────────────────────

const DSETDRV: u16 = 0x0E;
const DSETPATH: u16 = 0x3B;

fn gemdos1(func: u16, a0: u64) -> i32 {
    let args = TrapArgs {
        regs: [a0, 0, 0, 0, 0, 0, 0, 0],
        func: func as u64,
        svc_imm: TrapClass::Gemdos as u16,
        space: tos_api::uaccess::Space::Kernel,
    };
    tos_api::gemdos::dispatch(&args)
}

fn cstr(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

/// `p_setappdir` with a spec that holds no path separator clears the
/// application directory (`gempd.c:96-106`: `plast` never moves).
fn clear_app_dir() {
    tos_api::aes::resource::set_app_dir(b"NONE\0");
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

// ── findfile stage 1: the application directory ───────────────────────────

/// QED's first attempt: `strcpy(rsc_path, gl_appdir); strcat(rsc_path,
/// "qed.rsc")` (`sdk/apps/qed/src/rsc.c:50-53`) — a fully-qualified path,
/// with the application directory set to exactly that directory.
/// `findfile` (1) takes `sh_name(pspec)` = `QED.RSC` and appends it to
/// `p_appdir` (`gemshlib.c:412-423`), so the qualified spec resolves.
#[test]
fn a_qualified_path_in_the_application_directory_loads() {
    let _g = fs();
    tos_api::aes::resource::set_app_dir(b"C:\\APPS\\QED\\QED.NAP\0");
    assert_eq!(
        rsrc_load("C:\\APPS\\QED\\QED.RSC"),
        1,
        "rsrc_load must resolve the qualified path QED passes; EmuTOS \
         gemrslib.c:835-837 runs sh_find on it"
    );
    rsrc_free();
}

// ── findfile stage 2: the spec unchanged ──────────────────────────────────

/// `findfile` (2) searches for `pspec` **unchanged** (`gemshlib.c:426-431`).
/// That stage was missing entirely: NewTOS always pasted a prefix in front of
/// the name, so a qualified path could never match. Here the application
/// directory points somewhere *else*, so stage (1) must miss and stage (2)
/// must carry the load.
#[test]
fn a_qualified_path_outside_the_application_directory_loads() {
    let _g = fs();
    tos_api::aes::resource::set_app_dir(b"C:\\APPS\\QED\\SUB\\OTHER.NAP\0");
    assert_eq!(
        rsrc_load("C:\\APPS\\QED\\QED.RSC"),
        1,
        "findfile (2) searches the spec unchanged (gemshlib.c:426-431)"
    );
    rsrc_free();
}

/// The same stage for a *relative* spec: with the current directory on
/// `C:\APPS\QED`, `QED.RSC` is found there and not at the drive root.
#[test]
fn a_spec_relative_to_the_current_directory_loads() {
    let _g = fs();
    assert_eq!(gemdos1(DSETPATH, cstr("\\APPS\\QED").as_ptr() as u64), 0);
    assert_eq!(
        rsrc_load("QED.RSC"),
        1,
        "findfile (2) is the current directory (gemshlib.c:426-431)"
    );
    rsrc_free();
    assert_eq!(gemdos1(DSETPATH, cstr("\\").as_ptr() as u64), 0);
}

// ── findfile stage 3: the root of the current drive ───────────────────────

/// `findfile` (3) prefixes a single `\` and searches the **current drive**'s
/// root (`gemshlib.c:434-442`) — not a hardcoded `C:`. With no application
/// directory and the current directory two levels down, only stage (3) can
/// find `ROOTONLY.RSC`.
#[test]
fn a_bare_name_is_found_at_the_root_of_the_current_drive() {
    let _g = fs();
    assert_eq!(gemdos1(DSETPATH, cstr("\\APPS\\QED").as_ptr() as u64), 0);
    assert_eq!(
        rsrc_load("ROOTONLY.RSC"),
        1,
        "findfile (3) searches \\<name> on the current drive"
    );
    rsrc_free();
    assert_eq!(gemdos1(DSETPATH, cstr("\\").as_ptr() as u64), 0);
}

// ── the buffers EmuTOS sizes at MAXPATHLEN ────────────────────────────────

/// A 74-byte fully-qualified path, longer than the 64-byte buffer `rsrc_load`
/// composed into and than the old 64-byte application-directory store.
/// EmuTOS works in `MAXPATHLEN` (= 128, `include/sysconf.h:51-55`) buffers and
/// `p_appdir[LEN_ZPATH+2]` (= 116, `aes/struct.h:153`); a silent truncation
/// here turns a valid path into a missing file.
#[test]
fn a_path_longer_than_sixty_four_bytes_loads() {
    let _g = fs();
    assert!(DEEP_PATH.len() > 64, "the fixture must exceed the old buffer");
    assert_eq!(
        rsrc_load(DEEP_PATH),
        1,
        "a {}-byte path must not be silently truncated",
        DEEP_PATH.len()
    );
    rsrc_free();
}

/// The same file through stage (1), with an application directory of 66 bytes
/// — longer than the old `APP_DIR_CAP` (64). The spec is a bare name, so only
/// the *untruncated* application directory can resolve it.
#[test]
fn an_application_directory_longer_than_sixty_four_bytes_is_kept_whole() {
    let _g = fs();
    assert!(DEEP_DIR.len() > 64, "the fixture must exceed APP_DIR_CAP");
    let spec = cstr(DEEP_PATH);
    tos_api::aes::resource::set_app_dir(&spec);
    let mut got = [0u8; 128];
    let n = tos_api::aes::resource::app_dir(&mut got);
    assert_eq!(
        &got[..n],
        DEEP_DIR.as_bytes(),
        "set_app_dir truncated a {}-byte application directory",
        DEEP_DIR.len()
    );
    assert_eq!(
        rsrc_load("DEEP.RSC"),
        1,
        "findfile (1) must compose the whole application directory + name"
    );
    rsrc_free();
}

// ── the failure contract ──────────────────────────────────────────────────

/// All four stages miss → `int_out[0] = 0` (`gemrslib.c:836-837`).
#[test]
fn a_name_that_exists_nowhere_returns_zero() {
    let _g = fs();
    tos_api::aes::resource::set_app_dir(b"C:\\APPS\\QED\\QED.NAP\0");
    assert_eq!(rsrc_load("C:\\APPS\\QED\\NOSUCH.RSC"), 0);
    assert_eq!(rsrc_load("NOSUCH.RSC"), 0);
}
