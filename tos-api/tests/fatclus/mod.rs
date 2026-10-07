//! A FAT16 volume with **multi-sector clusters** in host memory, wired to the
//! TOS-API layer as its block device, with a log of every device request —
//! the fixture for the FS08 transfer-engine tests (`tests/gemdos_fs_curcl.rs`,
//! `tests/gemdos_fs_chain_steps.rs`).
//!
//! `tests/fatvol` has one-sector clusters, so it cannot tell a cluster step
//! from a sector step, and it counts sectors, not requests. This one has
//! 4-sector (2 KiB) clusters, a contiguous and a fragmented file with known
//! contents, and records each `read_blocks`/`write_blocks` call as
//! `(lba, sectors)`.

#![allow(dead_code)] // each test binary uses a different part of this

use std::sync::{Mutex, MutexGuard, OnceLock};

use tos_api::host::Host;
use tos_api::trap::{TrapArgs, TrapClass};

pub const SECTOR: usize = 512;
const PART_LBA: u32 = 64;
pub const CLSIZ: u32 = 4;
pub const CLSIZB: u32 = CLSIZ * SECTOR as u32;
const RESERVED: u32 = 1;
const NUM_FATS: u32 = 2;
const ROOT_ENTRIES: u32 = 512;
const RDLEN: u32 = ROOT_ENTRIES * 32 / SECTOR as u32;
/// > 4084 clusters, so the BPB parser makes it FAT16 (writable).
const NUMCL: u32 = 5000;
pub const FSIZ: u32 = ((NUMCL + 2) * 2).div_ceil(SECTOR as u32);
const FAT1: u32 = RESERVED;
const ROOT_SEC: u32 = RESERVED + NUM_FATS * FSIZ;
const DATA_SEC: u32 = ROOT_SEC + RDLEN;
const PART_SECTORS: u32 = DATA_SEC + NUMCL * CLSIZ;

/// `C:\SEQ.DAT`: 40 contiguous clusters, length not a multiple of a record.
pub const SEQ_CHAIN: &[u16] = &[
    10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
    33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
];
pub const SEQ_LEN: u32 = 40 * CLSIZB - 300;
pub const SEQ_SEED: u8 = 0x11;

/// `C:\FRAG.DAT`: 8 clusters in five contiguous runs
/// `[60 61] [90] [62] [120 121 122] [70]`.
pub const FRAG_CHAIN: &[u16] = &[60, 61, 90, 62, 120, 121, 122, 70];
pub const FRAG_LEN: u32 = 8 * CLSIZB - 100;
pub const FRAG_SEED: u8 = 0x5A;

/// The byte at offset `i` of a fixture file.
pub fn pattern(seed: u8, i: u32) -> u8 {
    (i.wrapping_mul(7).wrapping_add(i / 251) as u8) ^ seed
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

fn build_image() -> Vec<u8> {
    let total = (PART_LBA + PART_SECTORS) as usize;
    let mut img = vec![0u8; total * SECTOR];
    {
        let e = 0x1BE;
        img[e + 4] = 0x06;
        img[e + 8..e + 12].copy_from_slice(&PART_LBA.to_le_bytes());
        img[e + 12..e + 16].copy_from_slice(&PART_SECTORS.to_le_bytes());
        img[510] = 0x55;
        img[511] = 0xAA;
    }
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
    let link = |img: &mut Vec<u8>, chain: &[u16]| {
        for copy in 0..NUM_FATS {
            let fat = part + ((FAT1 + copy * FSIZ) as usize) * SECTOR;
            for (k, &cl) in chain.iter().enumerate() {
                let next = chain.get(k + 1).copied().unwrap_or(0xFFFF);
                let o = fat + cl as usize * 2;
                img[o..o + 2].copy_from_slice(&next.to_le_bytes());
            }
        }
    };
    for copy in 0..NUM_FATS {
        let fat = part + ((FAT1 + copy * FSIZ) as usize) * SECTOR;
        img[fat..fat + 2].copy_from_slice(&0xFFF8u16.to_le_bytes());
        img[fat + 2..fat + 4].copy_from_slice(&0xFFFFu16.to_le_bytes());
    }
    link(&mut img, SEQ_CHAIN);
    link(&mut img, FRAG_CHAIN);
    {
        let root = part + ROOT_SEC as usize * SECTOR;
        img[root..root + 32].copy_from_slice(&dirent(b"SEQ     DAT", 0x20, SEQ_CHAIN[0], SEQ_LEN));
        img[root + 32..root + 64]
            .copy_from_slice(&dirent(b"FRAG    DAT", 0x20, FRAG_CHAIN[0], FRAG_LEN));
    }
    let fill = |img: &mut Vec<u8>, chain: &[u16], len: u32, seed: u8| {
        for i in 0..len {
            let cl = chain[(i / CLSIZB) as usize] as u32;
            let at = part + ((DATA_SEC + (cl - 2) * CLSIZ) as usize) * SECTOR + (i % CLSIZB) as usize;
            img[at] = pattern(seed, i);
        }
    };
    fill(&mut img, SEQ_CHAIN, SEQ_LEN, SEQ_SEED);
    fill(&mut img, FRAG_CHAIN, FRAG_LEN, FRAG_SEED);
    img
}

/// One device request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Req {
    pub write: bool,
    pub lba: u64,
    pub sectors: u32,
}

pub struct LogDisk {
    img: Mutex<Vec<u8>>,
    log: Mutex<Vec<Req>>,
}

impl tos_api::bios::block::BlockDev for LogDisk {
    fn block_count(&self) -> u64 {
        (self.img.lock().unwrap().len() / SECTOR) as u64
    }
    fn read_blocks(&self, lba: u64, buf: &mut [u8]) -> Result<(), i32> {
        let n = buf.len() / SECTOR;
        let img = self.img.lock().unwrap();
        let off = lba as usize * SECTOR;
        if off + n * SECTOR > img.len() {
            return Err(tos_types::error::ESECNF);
        }
        buf.copy_from_slice(&img[off..off + n * SECTOR]);
        self.log.lock().unwrap().push(Req { write: false, lba, sectors: n as u32 });
        Ok(())
    }
    fn write_blocks(&self, lba: u64, buf: &[u8]) -> Result<(), i32> {
        let n = buf.len() / SECTOR;
        let mut img = self.img.lock().unwrap();
        let off = lba as usize * SECTOR;
        if off + n * SECTOR > img.len() {
            return Err(tos_types::error::ESECNF);
        }
        img[off..off + n * SECTOR].copy_from_slice(buf);
        self.log.lock().unwrap().push(Req { write: true, lba, sectors: n as u32 });
        Ok(())
    }
}

pub struct DiskHost {
    pub disk: LogDisk,
}

impl Host for DiskHost {
    fn console_write_byte(&self, _byte: u8) {}
    fn block(&self) -> Option<&dyn tos_api::bios::block::BlockDev> {
        Some(&self.disk)
    }
}

/// Absolute LBA of partition-relative sector `rel`.
pub fn part_lba(rel: u32) -> u64 {
    (PART_LBA + rel) as u64
}
/// Absolute LBA of the first sector of FAT copy `copy`.
pub fn fat_lba(copy: u32) -> u64 {
    part_lba(FAT1 + copy * FSIZ)
}
/// Absolute LBA of the first sector of data cluster `cl`.
pub fn cluster_lba(cl: u32) -> u64 {
    part_lba(DATA_SEC + (cl - 2) * CLSIZ)
}
/// Is `lba` inside either FAT copy?
pub fn is_fat(lba: u64) -> bool {
    lba >= fat_lba(0) && lba < fat_lba(NUM_FATS)
}
/// Is `lba` in the data area (clusters)?
pub fn is_data(lba: u64) -> bool {
    lba >= part_lba(DATA_SEC)
}

impl DiskHost {
    /// Forget the request log.
    pub fn clear_log(&self) {
        self.disk.log.lock().unwrap().clear();
    }
    /// The requests since the last [`DiskHost::clear_log`].
    pub fn log(&self) -> Vec<Req> {
        self.disk.log.lock().unwrap().clone()
    }
    /// One raw sector, bypassing the BDOS cache and the log.
    pub fn raw(&self, lba: u64) -> Vec<u8> {
        let img = self.disk.img.lock().unwrap();
        img[lba as usize * SECTOR..(lba as usize + 1) * SECTOR].to_vec()
    }
}

static HOST: OnceLock<&'static DiskHost> = OnceLock::new();
static FS_LOCK: Mutex<()> = Mutex::new(());

pub fn fs<'a>() -> (MutexGuard<'a, ()>, &'static DiskHost) {
    let guard = FS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let host = *HOST.get_or_init(|| {
        let host: &'static DiskHost = Box::leak(Box::new(DiskHost {
            disk: LogDisk { img: Mutex::new(build_image()), log: Mutex::new(Vec::new()) },
        }));
        let slot: &'static &'static dyn Host = Box::leak(Box::new(host as &'static dyn Host));
        tos_api::host::install_host(slot);
        assert_eq!(tos_api::bios::mount(), 1, "one FAT16 partition expected");
        host
    });
    (guard, host)
}

pub fn call(func: u16, regs: [u64; 8]) -> i32 {
    let args = TrapArgs {
        regs,
        func: func as u64,
        svc_imm: TrapClass::Gemdos as u16,
        space: tos_api::uaccess::Space::Kernel,
    };
    tos_api::gemdos::dispatch(&args)
}

pub fn cpath(p: &str) -> Vec<u8> {
    let mut v = p.as_bytes().to_vec();
    v.push(0);
    v
}

pub fn fopen(name: &str, mode: u64) -> i32 {
    let p = cpath(name);
    call(0x3D, [p.as_ptr() as u64, mode, 0, 0, 0, 0, 0, 0])
}
pub fn fcreate(name: &str) -> i32 {
    let p = cpath(name);
    call(0x3C, [p.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0])
}
pub fn fread(h: i32, buf: &mut [u8]) -> i32 {
    call(0x3F, [h as u64, buf.len() as u64, buf.as_mut_ptr() as u64, 0, 0, 0, 0, 0])
}
pub fn fwrite(h: i32, buf: &[u8]) -> i32 {
    call(0x40, [h as u64, buf.len() as u64, buf.as_ptr() as u64, 0, 0, 0, 0, 0])
}
pub fn fseek(off: i64, h: i32, mode: u16) -> i32 {
    call(0x42, [off as i32 as u32 as u64, h as u64, mode as u64, 0, 0, 0, 0, 0])
}
pub fn fclose(h: i32) -> i32 {
    call(0x3E, [h as u64, 0, 0, 0, 0, 0, 0, 0])
}
pub fn fdelete(name: &str) -> i32 {
    let p = cpath(name);
    call(0x41, [p.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0])
}

/// Read an open file to its end in chunks of `chunk` bytes.
pub fn read_all(h: i32, chunk: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; chunk];
    loop {
        let n = fread(h, &mut buf);
        assert!(n >= 0, "Fread -> {n}");
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n as usize]);
    }
    out
}

/// The expected contents of a fixture file.
pub fn expected(seed: u8, len: u32) -> Vec<u8> {
    (0..len).map(|i| pattern(seed, i)).collect()
}
