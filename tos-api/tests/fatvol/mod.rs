//! A FAT16 volume in host memory, wired to the TOS-API layer as its block
//! device, so the GEMDOS file-system calls can be driven from a host test.
//!
//! Shared by the integration tests in `tests/gemdos_*.rs`. Each of those is a
//! separate binary with its own copy of the TOS-API statics, so each installs
//! its own host and owns its own image.

#![allow(dead_code)] // each test binary uses a different part of this

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::host::Host;
use tos_api::trap::{TrapArgs, TrapClass};

// ── FAT16 volume, built in memory ─────────────────────────────────────────

const SECTOR: usize = 512;

/// LBA of the partition inside the image (the MBR's `start_lba`).
const PART_LBA: u32 = 64;

// Geometry of the synthesized volume. Cluster count must exceed
// `MAX_FAT12_CLUSTERS` (4084, `references/emutos/bios/blkdev.h:25`) or the BPB
// parser marks the volume FAT12, which is read-only here.
const CLSIZ: u32 = 1; // sectors per cluster
const RESERVED: u32 = 1;
const NUM_FATS: u32 = 2;
const ROOT_ENTRIES: u32 = 512;
const RDLEN: u32 = ROOT_ENTRIES * 32 / SECTOR as u32; // 32 sectors
const NUMCL: u32 = 5000;
const FSIZ: u32 = ((NUMCL + 2) * 2).div_ceil(SECTOR as u32); // 20 sectors
/// Partition-relative sector of the first FAT copy.
const FAT1: u32 = RESERVED;
/// Partition-relative sector of the root directory.
const ROOT_SEC: u32 = RESERVED + NUM_FATS * FSIZ;
/// Partition-relative sector of cluster 2.
const DATA_SEC: u32 = ROOT_SEC + RDLEN;
const PART_SECTORS: u32 = DATA_SEC + NUMCL * CLSIZ;

/// Build a 32-byte FAT directory entry.
fn dirent(name: &[u8; 11], attr: u8, cluster: u16, size: u32) -> [u8; 32] {
    let mut e = [0u8; 32];
    e[0..11].copy_from_slice(name);
    e[0x0B] = attr;
    e[0x16..0x18].copy_from_slice(&0x8000u16.to_le_bytes()); // 16:00:00
    e[0x18..0x1A].copy_from_slice(&0x5901u16.to_le_bytes()); // 2024-08-01
    e[0x1A..0x1C].copy_from_slice(&cluster.to_le_bytes());
    e[0x1C..0x20].copy_from_slice(&size.to_le_bytes());
    e
}

/// A whole disk image: MBR + one FAT16 partition holding
///
/// ```text
/// C:\HELLO.TXT     5 bytes, cluster 3
/// C:\SUB\          directory, cluster 2
/// C:\SUB\INNER.TXT 3 bytes, cluster 4
/// ```
fn build_image() -> Vec<u8> {
    let total = (PART_LBA + PART_SECTORS) as usize;
    let mut img = vec![0u8; total * SECTOR];

    // ── MBR ──
    {
        let e = 0x1BE;
        img[e + 4] = 0x06; // FAT16 partition type
        img[e + 8..e + 12].copy_from_slice(&PART_LBA.to_le_bytes());
        img[e + 12..e + 16].copy_from_slice(&PART_SECTORS.to_le_bytes());
        img[510] = 0x55;
        img[511] = 0xAA;
    }

    let part = PART_LBA as usize * SECTOR;

    // ── boot sector / BPB ──
    {
        let b = &mut img[part..part + SECTOR];
        b[0x0B..0x0D].copy_from_slice(&(SECTOR as u16).to_le_bytes()); // bytes/sector
        b[0x0D] = CLSIZ as u8; // sectors/cluster
        b[0x0E..0x10].copy_from_slice(&(RESERVED as u16).to_le_bytes());
        b[0x10] = NUM_FATS as u8;
        b[0x11..0x13].copy_from_slice(&(ROOT_ENTRIES as u16).to_le_bytes());
        // total_sectors_16 stays 0 -> the parser uses total_sectors_32
        b[0x16..0x18].copy_from_slice(&(FSIZ as u16).to_le_bytes());
        b[0x20..0x24].copy_from_slice(&PART_SECTORS.to_le_bytes());
        b[510] = 0x55;
        b[511] = 0xAA;
    }

    // ── FAT: clusters 2, 3, 4 are one-cluster chains ──
    for copy in 0..NUM_FATS {
        let fat = part + ((FAT1 + copy * FSIZ) as usize) * SECTOR;
        img[fat..fat + 2].copy_from_slice(&0xFFF8u16.to_le_bytes()); // media
        img[fat + 2..fat + 4].copy_from_slice(&0xFFFFu16.to_le_bytes());
        for cl in 2..5usize {
            let o = fat + cl * 2;
            img[o..o + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }
    }

    // ── root directory ──
    {
        let root = part + ROOT_SEC as usize * SECTOR;
        img[root..root + 32].copy_from_slice(&dirent(b"SUB        ", 0x10, 2, 0));
        img[root + 32..root + 64].copy_from_slice(&dirent(b"HELLO   TXT", 0x20, 3, 5));
    }

    // ── C:\SUB (cluster 2) ──
    {
        let sub = part + (DATA_SEC as usize) * SECTOR; // cluster 2
        img[sub..sub + 32].copy_from_slice(&dirent(b".          ", 0x10, 2, 0));
        img[sub + 32..sub + 64].copy_from_slice(&dirent(b"..         ", 0x10, 0, 0));
        img[sub + 64..sub + 96].copy_from_slice(&dirent(b"INNER   TXT", 0x20, 4, 3));
    }

    // ── file data ──
    {
        let c3 = part + (DATA_SEC as usize + 1) * SECTOR; // cluster 3
        img[c3..c3 + 5].copy_from_slice(b"hello");
        let c4 = part + (DATA_SEC as usize + 2) * SECTOR; // cluster 4
        img[c4..c4 + 3].copy_from_slice(b"in!");
    }

    img
}

// ── block device + host ───────────────────────────────────────────────────

pub struct RamDisk {
    img: Mutex<Vec<u8>>,
    pub reads: AtomicU32,
    /// Reads of the partition's boot sector, i.e. drive log-ins.
    pub boot_reads: AtomicU32,
    pub writes: AtomicU32,
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
        self.reads.fetch_add(n as u32, Ordering::Relaxed);
        if (lba as u32) == PART_LBA {
            self.boot_reads.fetch_add(1, Ordering::Relaxed);
        }
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
        self.writes.fetch_add(n as u32, Ordering::Relaxed);
        true
    }
}

pub struct DiskHost {
    pub disk: RamDisk,
}

impl Host for DiskHost {
    fn console_write_byte(&self, _byte: u8) {}
    fn block(&self) -> Option<&dyn tos_api::bios::block::BlockDev> {
        Some(&self.disk)
    }
}

static HOST: OnceLock<&'static DiskHost> = OnceLock::new();
/// The BDOS file system keeps its state in `UnsafeCell` statics with no
/// locking (it runs on the single-CPU, IRQ-off trap path), so the tests in
/// this binary must not overlap.
static FS_LOCK: Mutex<()> = Mutex::new(());

pub fn fs<'a>() -> (MutexGuard<'a, ()>, &'static DiskHost) {
    let guard = FS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let host = *HOST.get_or_init(|| {
        let host: &'static DiskHost = Box::leak(Box::new(DiskHost {
            disk: RamDisk {
                img: Mutex::new(build_image()),
                reads: AtomicU32::new(0),
                boot_reads: AtomicU32::new(0),
                writes: AtomicU32::new(0),
            },
        }));
        let slot: &'static &'static dyn Host = Box::leak(Box::new(host as &'static dyn Host));
        tos_api::host::install_host(slot);
        // Scan the MBR so `C:` exists. This also logs every drive out again
        // (`tos_api::bios::mount`), which is what makes the read counters
        // below start from a known state.
        assert_eq!(tos_api::bios::mount(), 1, "one FAT16 partition expected");
        host
    });
    (guard, host)
}

// ── GEMDOS call helpers ───────────────────────────────────────────────────

pub fn call(func: u16, regs: [u64; 8]) -> i32 {
    let args = TrapArgs {
        regs,
        func: func as u64,
        svc_imm: TrapClass::Gemdos as u16,
    };
    tos_api::gemdos::dispatch(&args)
}

pub fn gemdos1(func: u16, a0: u64) -> i32 {
    call(func, [a0, 0, 0, 0, 0, 0, 0, 0])
}

pub fn gemdos2(func: u16, a0: u64, a1: u64) -> i32 {
    call(func, [a0, a1, 0, 0, 0, 0, 0, 0])
}

pub const FSETDTA: u16 = 0x1A;
pub const FSFIRST: u16 = 0x4E;
pub const FSNEXT: u16 = 0x4F;
pub const FOPEN: u16 = 0x3D;
pub const FCLOSE: u16 = 0x3E;
pub const FCREATE: u16 = 0x3C;
pub const FDELETE: u16 = 0x41;
pub const DCREATE: u16 = 0x39;
pub const DGETPATH: u16 = 0x47;
pub const DSETDRV: u16 = 0x0E;

pub const E_OK: i32 = 0;
pub const EFILNF: i32 = -33;
pub const EPTHNF: i32 = -34;
pub const ENMFIL: i32 = -49;

/// TeraDesk's `FA_ANY` — every attribute but the volume label
/// (`sdk/apps/teradesk/xfilesys.c:1173`, seen as `attr=0x37` in the trace).
pub const FA_ANY: u64 = 0x37;

/// A 44-byte DTA plus a NUL-terminated path, so a call looks exactly like the
/// application's.
pub struct Dta([u8; 44]);

impl Dta {
    pub fn new() -> Dta {
        Dta([0u8; 44])
    }
    pub fn ptr(&mut self) -> u64 {
        self.0.as_mut_ptr() as u64
    }
    /// `d_fname` — the found entry's name (offset 30, NUL-terminated).
    pub fn name(&self) -> String {
        let n = &self.0[30..44];
        let end = n.iter().position(|&c| c == 0).unwrap_or(n.len());
        String::from_utf8_lossy(&n[..end]).into_owned()
    }
}

pub fn cpath(p: &str) -> Vec<u8> {
    let mut v = p.as_bytes().to_vec();
    v.push(0);
    v
}

