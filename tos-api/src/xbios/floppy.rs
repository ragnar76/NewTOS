//! XBIOS floppy / DMA group.
//!
//! Spec: `docs/api/xbios/_floppy-stubs.md`. The NewTOS profile has no
//! floppy controller and no Atari ACSI/DMA bus, so the transfer functions
//! (`Floprd`/`Flopwr`/`Flopfmt`/`Flopver`/`Floprate`/`DMAread`/`DMAwrite`)
//! return [`EUNDEV`] — "device not present" — which is the canonical TOS
//! signal a program checks for to detect-and-fall-back (the dispatcher
//! wires that group directly, see [`super::dispatch`]).
//!
//! `Protobt` (0x12) is the one member that does real work: it is pure
//! software (build/patch a 512-byte Atari boot-sector image), needs no
//! hardware, and is implemented in full here, matching EmuTOS
//! `bios/floppy.c:690-736` byte-for-byte.

use crate::trap::TrapArgs;

/// One 512-byte logical sector.
const SECTOR_SIZE: usize = 512;

/// Boot-sector field offsets (`struct bs`, `bios/blkdev.h:43-60`).
const OFF_SERIAL: usize = 0x08; // serial[3]
const OFF_BPB: usize = 0x0B; // bps..hid (the copied BPB template)
const OFF_CKSUM: usize = 0x1FE; // cksum[2] — last two bytes of the sector

/// Length of the `struct _protobt` BPB template copied at [`OFF_BPB`]:
/// `bps..hid` = 19 bytes (`bios/floppy.c:662-674`, `PACKED`, no padding).
const PROTOBT_LEN: usize = 19;

/// One `struct _protobt` geometry (`bios/floppy.c:662-687`). Serialised
/// little-endian ("intel words") into the boot sector's BPB area.
struct ProtobtGeom {
    bps: u16,   // bytes per sector
    spc: u8,    // sectors per cluster
    res: u16,   // reserved sectors
    fat: u8,    // number of FATs
    dir: u16,   // root-directory entries
    sec: u16,   // total sectors
    media: u8,  // media descriptor byte
    spf: u16,   // sectors per FAT
    spt: u16,   // sectors per track
    sides: u16, // number of sides
    hid: u16,   // hidden sectors
}

impl ProtobtGeom {
    /// Serialise into the 19-byte little-endian BPB image, exactly the
    /// on-disk order of `struct _protobt` (`bios/floppy.c:662-674`).
    const fn to_bytes(&self) -> [u8; PROTOBT_LEN] {
        [
            self.bps as u8,
            (self.bps >> 8) as u8,
            self.spc,
            self.res as u8,
            (self.res >> 8) as u8,
            self.fat,
            self.dir as u8,
            (self.dir >> 8) as u8,
            self.sec as u8,
            (self.sec >> 8) as u8,
            self.media,
            self.spf as u8,
            (self.spf >> 8) as u8,
            self.spt as u8,
            (self.spt >> 8) as u8,
            self.sides as u8,
            (self.sides >> 8) as u8,
            self.hid as u8,
            (self.hid >> 8) as u8,
        ]
    }
}

/// The seven `protobt_data[]` geometries (`bios/floppy.c:676-687`): an
/// EmuTOS extension of the classic four-entry Atari ST table. Valid
/// `disktype` values are `0..=6`; index → geometry:
/// 0 = 360K SS, 1 = 720K DS, 2 = 720K DS-5spf, 3 = 1.44M DS,
/// 4 = 2.88M HD, 5 = 5.76M ED, 6 = 800K.
const PROTOBT_DATA: [ProtobtGeom; 7] = [
    ProtobtGeom { bps: SECTOR_SIZE as u16, spc: 1, res: 1, fat: 2, dir: 64,  sec: 360,  media: 0xfc, spf: 2,  spt: 9,  sides: 1, hid: 0 },
    ProtobtGeom { bps: SECTOR_SIZE as u16, spc: 2, res: 1, fat: 2, dir: 112, sec: 720,  media: 0xfd, spf: 2,  spt: 9,  sides: 2, hid: 0 },
    ProtobtGeom { bps: SECTOR_SIZE as u16, spc: 2, res: 1, fat: 2, dir: 112, sec: 720,  media: 0xf9, spf: 5,  spt: 9,  sides: 1, hid: 0 },
    ProtobtGeom { bps: SECTOR_SIZE as u16, spc: 2, res: 1, fat: 2, dir: 112, sec: 1440, media: 0xf9, spf: 5,  spt: 9,  sides: 2, hid: 0 },
    ProtobtGeom { bps: SECTOR_SIZE as u16, spc: 2, res: 1, fat: 2, dir: 224, sec: 2880, media: 0xf0, spf: 5,  spt: 18, sides: 2, hid: 0 },
    ProtobtGeom { bps: SECTOR_SIZE as u16, spc: 2, res: 1, fat: 2, dir: 224, sec: 5760, media: 0xf0, spf: 10, spt: 36, sides: 2, hid: 0 },
    ProtobtGeom { bps: SECTOR_SIZE as u16, spc: 2, res: 1, fat: 2, dir: 112, sec: 1600, media: 0xf9, spf: 5,  spt: 10, sides: 2, hid: 0 },
];

/// Whole-sector word checksum, exactly EmuTOS `compute_cksum`
/// (`bios/blkdev.c:69-75`): the 16-bit-wrapping sum of all 256
/// **big-endian** 16-bit words of the 512-byte sector. The Atari boot
/// sector is a big-endian byte image, so word `i` is
/// `buf[2i] << 8 | buf[2i+1]`; the executable marker `0x1234` is defined
/// against this big-endian sum, so it must be computed big-endian even on
/// our little-endian AArch64 target (observable contract, not an m68k-ism).
fn compute_cksum(buf: &[u8]) -> u16 {
    let mut sum: u16 = 0;
    let mut i = 0;
    while i < SECTOR_SIZE {
        let word = ((buf[i] as u16) << 8) | (buf[i + 1] as u16);
        sum = sum.wrapping_add(word);
        i += 2;
    }
    sum
}

/// `Protobt` (0x12) — build/patch a 512-byte Atari boot-sector prototype in
/// the caller's buffer. Pure software; no hardware needed. Void → 0.
///
/// ABI (`xbios_v_llww`, `docs/api/xbios/_floppy-stubs.md` §Native ABI):
/// `x0` = `buf` (`UBYTE *`), `x1` = `serialno` (LONG), `x2` = `disktype`
/// (WORD), `x3` = `execflag` (WORD).
///
/// Behaviour (EmuTOS `bios/floppy.c:690-736`):
/// - Reads the *current* executable status from the existing whole-sector
///   checksum (`== 0x1234`) before touching anything.
/// - **serialno:** applied whenever `serialno >= 0`; `serialno >= 0x01000000`
///   generates a random 24-bit serial via XBIOS `Random` (0x11, the same
///   source), otherwise the literal value is used. The three serial bytes
///   are stored **little-endian** into `serial[0..2]` at offset 0x08.
/// - **disktype:** `0..=6` copies the whole 19-byte `_protobt` BPB template
///   to offset 0x0B; `< 0` or out of range leaves the BPB unchanged.
/// - **execflag:** `< 0` keeps the current executable status; then the
///   `cksum` word (offset 0x1FE, big-endian) is recomputed so the
///   whole-sector checksum is **0x1234** (executable) or **0x1235**
///   (non-executable — Falcon/TT TOS "hyper-compatibility", `:723-729`).
pub fn protobt(args: &TrapArgs) -> i64 {
    let buf_ptr = args.arg_u64(0);
    let serial = args.arg_i32(1); // LONG
    let disktype = args.arg_i16(2);
    let mut execflag = args.arg_i16(3);
    if buf_ptr == 0 {
        return 0;
    }

    // `buf_ptr` is the caller's 512-byte boot-sector buffer, read and
    // written in place: in the caller's space, checked readable and
    // writable with the program's rights and reached through its kernel
    // alias (ARCH03 3a, `Space::kptr`); a buffer the program may not use
    // ends the call and the program. An in-kernel caller's buffer is a
    // kernel address.
    let Ok(kbuf) = args.space.kptr(buf_ptr, SECTOR_SIZE as u64, true) else {
        return i64::from(crate::uaccess::FAULTED);
    };
    // SAFETY: `kbuf..kbuf+512` was just checked (or is a kernel caller's own
    // buffer) and is reached through the kernel mapping; single-CPU trap.
    let buf = unsafe { core::slice::from_raw_parts_mut(kbuf as *mut u8, SECTOR_SIZE) };

    // 1. Current executable status from the existing checksum.
    let is_exec = compute_cksum(buf) == 0x1234;

    // 2. Serial number: stamp whenever non-negative; >= 0x01000000 → random.
    if serial >= 0 {
        let value: i32 = if serial >= 0x0100_0000 {
            // Same 24-bit source as XBIOS Random (0x11).
            crate::xbios::system::random() as i32
        } else {
            serial
        };
        buf[OFF_SERIAL] = value as u8;
        buf[OFF_SERIAL + 1] = (value >> 8) as u8;
        buf[OFF_SERIAL + 2] = (value >> 16) as u8;
    }

    // 3. BPB template for a valid disktype (0..=6).
    if disktype >= 0 && (disktype as usize) < PROTOBT_DATA.len() {
        let bytes = PROTOBT_DATA[disktype as usize].to_bytes();
        buf[OFF_BPB..OFF_BPB + PROTOBT_LEN].copy_from_slice(&bytes);
    }

    // 4. Executable flag → checksum trimming (EmuTOS `:727-735`).
    if execflag < 0 {
        execflag = if is_exec { 1 } else { 0 };
    }
    buf[OFF_CKSUM] = 0;
    buf[OFF_CKSUM + 1] = 0;
    let cksum = 0x1234u16.wrapping_sub(compute_cksum(buf));
    buf[OFF_CKSUM] = (cksum >> 8) as u8; // HIBYTE
    buf[OFF_CKSUM + 1] = cksum as u8; // LOBYTE
    if execflag == 0 {
        // Non-executable: bump the low byte so the whole-sector sum is
        // 0x1235 (EmuTOS increments `cksum[1]` as a byte, `:734-735`).
        buf[OFF_CKSUM + 1] = buf[OFF_CKSUM + 1].wrapping_add(1);
    }
    0
}
