//! ELF-to-slot loader — multi-page TPA.
//!
//! Combines [`super::elf`] (parser), [`super::arena`] (the pool-backed
//! per-process TPA) and [`super::mmu`] (per-slot page tables) into a
//! single function: given an ELF blob and a process slot, allocate a
//! multi-page Transient Program Area, copy the program image into it,
//! zero its BSS, write the [`BasePage`], and install the low-half
//! mapping that lets EL0 fetch and run from it.
//!
//! ## TPA layout
//!
//! ```text
//!   USER_VA_BASE (0) ─┐ BasePage           (sizeof BasePage = 256 B)
//!                     │ program image      (PT_LOAD segments, may span
//!                     │                     many pages: text/rodata/data)
//!        image_end    │ BSS                (zeroed: mem_size − file_size)
//!                     │ heap               (free TPA, for malloc)
//!                     │  ⋮
//!   USER_TPA_TOP      ┘ SP_EL0 / p_hitpa   (stack grows down from here)
//! ```
//!
//! The whole `[USER_VA_BASE, USER_TPA_TOP)` window (4 MiB, see
//! [`super::arena::TPA_SIZE`]) is a single pool block mapped page-by-
//! page into the slot's `TTBR0_EL1`. The basepage carries the TPA and
//! segment extents (`p_lowtpa`/`p_hitpa`/`p_tbase`/…/`p_bbase`) so a
//! libc/crt0 can find its heap and stack.
//!
//! ## Deliberate simplifications
//!
//! - **Uniform `UserRwExec` pages.** Every TPA page is mapped
//!   read/write/execute ([`super::mmu::PageFlags::UserRwExec`]) rather
//!   than splitting `.text` RX from data/BSS/stack RW-NX. A per-segment
//!   permission split is a later hardening step; it does not change the
//!   observable program contract.
//! - **Whole-TPA zero.** The loader zeroes the entire TPA before
//!   copying segments, so BSS, heap and stack all start zeroed and no
//!   stale pool contents leak into the new process. The SDK `crt0`
//!   therefore does **not** zero BSS itself.
//! - **Fixed TPA size.** [`USER_TPA_TOP`] is a compile-time constant;
//!   a per-`Pexec` size (from a stack request) is a future refinement.

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::sync::atomic::Ordering;

use tos_types::{BasePage, Long, MX_PREFTTRAM, NUMSTD, PDCLSIZE, PF_STANDARD};

use crate::bdos::fs::FileImage;

use super::arena;
use super::elf::{self, ElfImage, LoadSegment, ParseError, PF_W, PF_X};
use super::mmu::{self, MapError, PageFlags};
use super::PROCESS_TABLE;

/// High-half kernel VA offset (mirror of [`super::arena`]'s constant and
/// `kernel::arch::aarch64::mmu::VA_OFFSET`). The whole of DRAM is mapped
/// read/write in the high half at `phys + VA_OFFSET`, so a BDOS-pool
/// physical address (an Atari `Long`, always in the low 2 GiB) is
/// reachable from the kernel at this VA — which is how the loader writes
/// a freshly-`Mxalloc`'d environment block into pool memory.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

/// Kernel VA that aliases physical address `pa`. Inverse of
/// [`super::arena`]'s `va_to_pa`.
#[inline(always)]
const fn pa_to_kva(pa: u64) -> u64 {
    pa.wrapping_add(VA_OFFSET)
}

/// Failure modes for [`load_to_slot`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LoadError {
    /// The blob did not parse as a valid AArch64 ELF executable.
    Parse(ParseError),
    /// `slot` is `>= MAX_PROCESSES`.
    InvalidSlot,
    /// A `PT_LOAD` segment lies below the reserved low region. The first
    /// page of the TPA is reserved for the basepage and the classic
    /// low-memory system area (sysvars/OSHEADER/cookie-jar); segments must
    /// start at `USER_VA_BASE + RESERVED_LOW_END` (= 0x1000) or later.
    /// Anything below that produces this error (B1 / Option 1).
    SegmentVaOutOfRange,
    /// A `PT_LOAD` segment's `vaddr + mem_size` overruns the usable
    /// TPA image region (`USER_TPA_TOP - USER_STACK_MIN`), i.e. the
    /// program image + BSS would collide with the reserved stack.
    ProgramTooLarge,
    /// `super::mmu::map_page` failed (validation error inside the
    /// page-table builder).
    MapPage(MapError),
    /// The environment block could not be allocated from the BDOS pool
    /// (`Mxalloc` returned 0). EmuTOS `alloc_env` failure →
    /// `ENSMEM` (`bdos/proc.c:284-288`).
    NoEnvMemory,
    /// The per-process TPA could not be allocated from the BDOS pool
    /// (`Mxalloc` returned 0). Maps to `ENSMEM` — the classic Atari
    /// "not enough memory to load the program" (EmuTOS `bdos/proc.c`
    /// `xexec` → `ENSMEM`).
    NoTpaMemory,
}

/// How `Pexec` supplies the child's environment block. Mirrors the
/// `env` argument of EmuTOS `xexec` / `alloc_env` (`bdos/proc.c:413`).
#[derive(Copy, Clone, Debug)]
pub enum EnvSpec<'a> {
    /// No environment is set — `p_env` stays `0`. Used by the
    /// kernel-launch selftests (`load_to_slot`), which never handed the
    /// child an env and predate this plumbing.
    None,
    /// Copy this explicit environment image verbatim. The slice is the
    /// caller's block **including** its terminating double-NUL, i.e.
    /// `envsize` bytes (`bdos/proc.c:150`).
    Explicit(&'a [u8]),
    /// `env == NULL`: duplicate the parent's `run->p_env`
    /// (`bdos/proc.c:419-420`). The root process carries an empty
    /// double-NUL env (`bdos/bdosmain.c:341`), so with no running parent
    /// (or a parent whose `p_env` is 0) the child inherits an empty env.
    Inherit,
}

/// The empty environment: a single double-NUL. Matches EmuTOS's
/// `double_nul` static (`bdos/bdosmain.c:341`).
static EMPTY_ENV: [u8; 2] = [0, 0];

impl From<ParseError> for LoadError {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}
impl From<MapError> for LoadError {
    fn from(e: MapError) -> Self {
        Self::MapPage(e)
    }
}

/// Result of a successful load — what the caller needs to drop the
/// CPU into EL0.
#[derive(Copy, Clone, Debug)]
pub struct LoadedImage {
    /// Process slot the image was loaded into.
    pub slot: usize,
    /// User VA of the program's entry point (= ELF `e_entry`).
    pub entry_va: u64,
    /// Phys of `slot`'s `L0` table — feed this to
    /// [`super::mmu::install_ttbr0`] right before the `eret`.
    pub l0_pa: u64,
    /// User VA of the [`BasePage`] populated by
    /// [`populate_basepage`] — always equals `USER_VA_BASE` (the TPA
    /// starts with its basepage).
    pub basepage_va: u64,
    /// Initial `SP_EL0` for the child: the mapped TPA top
    /// ([`USER_TPA_TOP`]), 4-KiB- and 16-byte-aligned per AAPCS64.
    /// The stack grows down from here; the kernel launch path sets
    /// `SP_EL0` to this and the SDK `crt0` re-derives the same value
    /// from the basepage's `p_hitpa`.
    pub sp_el0: u64,
}

/// Base of the user-space VA window and the basepage VA — every
/// process's TPA starts here (`USER_VA_BASE + 0` holds the basepage,
/// the program image begins at `USER_VA_BASE + sizeof(BasePage)`).
pub const USER_VA_BASE: u64 = 0x0;

/// Exclusive top of the per-process TPA (= initial `SP_EL0` and the
/// basepage's `p_hitpa`). The TPA maps `[USER_VA_BASE, USER_TPA_TOP)`;
/// the stack grows down from here. Fixed today at
/// `USER_VA_BASE + arena::TPA_SIZE` (4 MiB); a per-`Pexec` size would
/// make this a per-process value carried in [`LoadedImage`].
pub const USER_TPA_TOP: u64 = USER_VA_BASE + arena::TPA_SIZE;

/// Minimum stack reserved below [`USER_TPA_TOP`]: the program image +
/// BSS may not grow into this window (they would collide with the
/// stack). The heap lives between the image top and this reserve.
const USER_STACK_MIN: u64 = 0x1_0000;

/// Number of bytes at the start of the TPA reserved for the
/// [`BasePage`].
const BASEPAGE_RESERVED: u64 = core::mem::size_of::<BasePage>() as u64;

/// End of the per-process reserved low region: the first page (4 KiB) of the
/// TPA holds the [`BasePage`] (`[0x000, 0x100)`) and the classic TOS
/// low-memory system area (`[0x400, 0x670)`: sysvars + OSHEADER + cookie jar,
/// written by [`populate_sysvars`]). ELF `PT_LOAD` segments must place
/// themselves at or after this address so they never overlap it — the SDK
/// links native apps here (`sdk/newtos.ld` base `0x1000`). B1 / Option 1,
/// `docs/analysis/bios-audit/b1-lowmem-feasibility.md` §Option 1.
const RESERVED_LOW_END: u64 = 0x1000;

// The reserved low region must fit inside the first TPA page (so `page 0`
// covers the BasePage + sysvar area) and leave the classic sysvar block
// (`[0x400, 0x800)`) below the image base.
const _: () = assert!(BASEPAGE_RESERVED <= RESERVED_LOW_END);
const _: () = assert!(SYSVAR_AREA_END <= RESERVED_LOW_END);

// The image window must leave room for both the reserved low region and the
// minimum stack reserve inside the TPA.
const _: () = assert!(RESERVED_LOW_END + USER_STACK_MIN < arena::TPA_SIZE);

/// Where the program image comes from. There is **no** fixed
/// whole-file staging buffer: only the ELF header + program-header table
/// pass through a small buffer ([`HEADER_BUF`]); every `PT_LOAD` payload
/// streams straight into the mapped TPA, so a program is capped only by
/// the TPA size, never by an intermediate buffer.
///
/// * [`ImageSource::Memory`] — an in-memory blob: an embedded registry
///   program or a selftest image (`&'static [u8]`). The whole blob is
///   already resident, so it doubles as its own header window.
/// * [`ImageSource::File`] — a FAT file streamed by byte offset via
///   [`FileImage::read_at`]; the header is read into [`HEADER_BUF`] and
///   each segment's `[p_offset, p_offset + p_filesz)` range is read
///   directly into the TPA VA.
pub enum ImageSource<'a> {
    /// In-memory ELF blob (embedded program / selftest image).
    Memory(&'a [u8]),
    /// FAT file streamed by offset.
    File(FileImage),
}

impl ImageSource<'_> {
    /// Read exactly `dst.len()` bytes of the image starting at byte
    /// `offset` into `dst`. `false` on a short read or IO error (a
    /// segment whose file range is not fully backed by the image); the
    /// parser has already validated `offset + dst.len() <= total_len`,
    /// so a well-formed image always fills `dst` completely.
    fn read_exact_at(&self, offset: u64, dst: &mut [u8]) -> bool {
        match self {
            Self::Memory(b) => {
                let Some(end) = (offset as usize).checked_add(dst.len()) else {
                    return false;
                };
                match b.get(offset as usize..end) {
                    Some(src) => {
                        dst.copy_from_slice(src);
                        true
                    }
                    None => false,
                }
            }
            Self::File(f) => {
                let Ok(off) = u32::try_from(offset) else {
                    return false;
                };
                matches!(f.read_at(off, dst), Some(n) if n == dst.len())
            }
        }
    }
}

/// Small buffer for the ELF header + program-header table of a streamed
/// file (never a segment payload). 8 KiB holds `Ehdr` (64 B) plus up to
/// ~145 program headers — orders of magnitude more than any real TOS
/// executable, which emit `phoff == 64` and a handful of phdrs.
const HEADER_BUF_SIZE: usize = 8 * 1024;
struct HeaderBuf(UnsafeCell<[u8; HEADER_BUF_SIZE]>);
// SAFETY: accessed only from the single-CPU, BDOS-locked trap path.
unsafe impl Sync for HeaderBuf {}
static HEADER_BUF: HeaderBuf = HeaderBuf(UnsafeCell::new([0u8; HEADER_BUF_SIZE]));

/// Load an ELF blob into the given process slot.
///
/// Sequence:
/// 1. Parse the blob via [`elf::parse`].
/// 2. For each `PT_LOAD` segment, validate it fits in the single
///    user page and copy file bytes into that page; zero the BSS
///    tail (`mem_size - file_size`).
/// 3. Initialise the slot's page tables and install one mapping
///    `(USER_VA_BASE → user-page-phys, UserRwExec)`.
/// 4. Return a [`LoadedImage`] ready for the caller's eret.
///
/// # Safety
///
/// Caller holds the BDOS giant lock; the slot must not currently
/// be the active TTBR0 (or the TLB must be flushed before the
/// next user-space access).
pub unsafe fn load_to_slot(
    elf_bytes: &[u8],
    slot: usize,
) -> Result<LoadedImage, LoadError> {
    // Legacy 2-arg entry (kernel-launch D3/D6/D7 selftests): no command
    // tail, no environment — the child's `p_cmdlin` and `p_env` stay
    // zero, exactly as before this plumbing landed.
    // SAFETY: forwarded — same BDOS-lock / trap-context contract.
    unsafe { load_to_slot_with(elf_bytes, slot, &[], EnvSpec::None, false) }
}

/// Load a FAT [`FileImage`] into `slot`, streaming each `PT_LOAD`
/// segment straight from the file into the mapped TPA — the `Pexec`
/// file-path entry. Segments never pass through a fixed buffer, so the
/// program is limited only by the TPA size, not by any load-buffer cap.
/// See [`load_to_slot_with`] for the `tail` / `env` / ownership
/// semantics (identical; only the image source differs).
///
/// # Safety
///
/// Caller holds the BDOS giant lock; see [`load_to_slot`].
pub unsafe fn load_file_to_slot(
    file: FileImage,
    slot: usize,
    tail: &[u8],
    env: EnvSpec<'_>,
    env_owned_by_child: bool,
) -> Result<LoadedImage, LoadError> {
    // SAFETY: forwarded — same BDOS-lock / trap-context contract.
    unsafe {
        load_from_source(ImageSource::File(file), slot, tail, env, env_owned_by_child)
    }
}

/// Load an ELF blob into `slot`, additionally populating the child's
/// command tail (`p_cmdlin`) and environment (`p_env`) per `Pexec`.
///
/// This is the full `Pexec`-path entry; [`load_to_slot`] is the
/// tail-/env-less legacy wrapper. See [`load_to_slot`] for the load
/// sequence; the only additions are:
///
///   * **Command tail** (`tail`): copied verbatim into `p_cmdlin`
///     (offset 0x80), truncated to 127 bytes, NUL-terminated —
///     EmuTOS `init_pd_fields` (`bdos/proc.c:378-383`). The classic
///     Atari image is a 1-byte length followed by the text; the copy
///     is byte-for-byte, so `p_cmdlin[0]` is whatever the caller put
///     there (the length under the convention).
///   * **Environment** (`env`): allocated from the BDOS pool and copied
///     verbatim — EmuTOS `alloc_env` (`bdos/proc.c:413-431`). `p_env`
///     receives the pool address of the copy.
///   * **Ownership** (`env_owned_by_child`): when `true` (PE_LOADGO),
///     the env block is re-tagged to the child so the child's `Pterm`
///     frees it (`owner = p`, `bdos/proc.c:305`); when `false`
///     (PE_LOAD) it keeps the allocating-process cookie (`owner =
///     run`).
///
/// # Safety
///
/// Caller holds the BDOS giant lock; see [`load_to_slot`].
pub unsafe fn load_to_slot_with(
    elf_bytes: &[u8],
    slot: usize,
    tail: &[u8],
    env: EnvSpec<'_>,
    env_owned_by_child: bool,
) -> Result<LoadedImage, LoadError> {
    // SAFETY: forwarded — same BDOS-lock / trap-context contract.
    unsafe {
        load_from_source(
            ImageSource::Memory(elf_bytes),
            slot,
            tail,
            env,
            env_owned_by_child,
        )
    }
}

/// Core loader shared by the in-memory ([`load_to_slot_with`]) and
/// FAT-file ([`load_file_to_slot`]) entry points. Parses the header,
/// allocates + owns the TPA, and streams the segments into it.
///
/// # Safety
///
/// Caller holds the BDOS giant lock; the slot must not currently be the
/// active TTBR0 (or the TLB must be flushed before the next user access).
unsafe fn load_from_source(
    src: ImageSource<'_>,
    slot: usize,
    tail: &[u8],
    env: EnvSpec<'_>,
    env_owned_by_child: bool,
) -> Result<LoadedImage, LoadError> {
    if slot >= super::MAX_PROCESSES {
        return Err(LoadError::InvalidSlot);
    }

    // 1. Parse the header (before touching the pool, so a malformed image
    //    costs no allocation). An in-memory blob is its own header window;
    //    a streamed file reads its header + phdrs into `HEADER_BUF` (never
    //    a segment payload — those go straight into the TPA in step 3).
    let img: ElfImage = match &src {
        ImageSource::Memory(blob) => elf::parse(blob, blob.len() as u64)?,
        ImageSource::File(file) => {
            // SAFETY: single BDOS-locked trap path; the buffer is not
            // aliased while the returned borrow is live.
            let hdr = unsafe { &mut *HEADER_BUF.0.get() };
            let n = file
                .read_at(0, hdr)
                .ok_or(LoadError::Parse(ParseError::TooShort))?;
            elf::parse(&hdr[..n], file.len() as u64)?
        }
    };

    // 2. Allocate the per-process TPA from the BDOS pool and re-tag it
    //    to the child basepage cookie (= the TPA base kernel VA), so
    //    the child's `Pterm` `free_all_owned` sweep reclaims it — the
    //    EmuTOS m_own model for the basepage MD (`bdos/proc.c:305`).
    // SAFETY: BDOS lock held; single-CPU pre-test.
    let tpa = unsafe { arena::alloc_tpa() }.ok_or(LoadError::NoTpaMemory)?;
    // SAFETY: `raw_start` is on the allocated list; `base_kva` is a
    // pointer-equality cookie only (never dereferenced by the sweep).
    unsafe {
        crate::bdos::mem::traps::set_owner(tpa.raw_start, tpa.base_kva.cast());
    }

    // 3. Populate + map; free the TPA on any failure — no `Pterm` runs
    //    for a load that never launched, so we must not leak the block.
    // SAFETY: BDOS lock held; `tpa` is a fresh, exclusively-owned
    // page-aligned region.
    match unsafe {
        populate_and_map(&src, slot, &img, tail, env, env_owned_by_child, &tpa)
    } {
        Ok(loaded) => Ok(loaded),
        Err(e) => {
            // SAFETY: BDOS lock held; `raw_start` is still allocated.
            unsafe { arena::free_tpa(tpa.raw_start) };
            Err(e)
        }
    }
}

/// Stream the ELF image into `tpa`, zero its BSS, allocate the
/// environment, write the basepage, sync the code caches, install the
/// per-process page tables, and publish the slot metadata.
///
/// Factored out of [`load_from_source`] so the caller can free the TPA
/// on any error (the `?`/early-return points all propagate here).
///
/// # Safety
///
/// Caller holds the BDOS giant lock; `tpa` is a fresh page-aligned
/// pool region owned by the child; `slot` is bounds-checked.
unsafe fn populate_and_map(
    src: &ImageSource<'_>,
    slot: usize,
    img: &ElfImage,
    tail: &[u8],
    env: EnvSpec<'_>,
    env_owned_by_child: bool,
    tpa: &arena::Tpa,
) -> Result<LoadedImage, LoadError> {
    // Whole-TPA mutable byte view via the kernel high-half alias.
    // SAFETY: the pool block backs `TPA_SIZE` contiguous bytes at
    // `base_kva`, mapped RW in the high half; single-CPU under the lock.
    let tpa_bytes = unsafe {
        core::slice::from_raw_parts_mut(tpa.base_kva, arena::TPA_SIZE as usize)
    };

    // Zero the whole TPA: BSS, heap and stack all start zeroed and no
    // stale pool contents leak into the new process (see module docs).
    tpa_bytes.fill(0);

    // Stream each PT_LOAD segment's file bytes directly into the mapped
    // TPA at its VA — for a FAT file, straight from disk into the TPA VA
    // with no fixed intermediate buffer (the segment can be any size up
    // to the TPA). Track the highest image byte so the cache maintenance
    // below covers exactly text+data (not the heap).
    let mut image_top = BASEPAGE_RESERVED;
    for seg in img.segments() {
        validate_segment(seg)?;

        let dest_off = (seg.vaddr - USER_VA_BASE) as usize;
        let file_size = seg.file_size as usize;

        if file_size > 0 {
            let dst = &mut tpa_bytes[dest_off..dest_off + file_size];
            if !src.read_exact_at(seg.file_offset, dst) {
                return Err(LoadError::Parse(ParseError::SegmentOutOfRange));
            }
        }
        // The BSS tail (`mem_size - file_size`) is already zero from the
        // whole-TPA fill above, so no per-segment re-zero is needed.
        image_top = image_top.max(seg.vaddr + seg.mem_size);
    }

    // Allocate + copy the environment (EmuTOS `alloc_env`), before the
    // basepage so `p_env` is stamped in one pass. The child cookie is
    // the TPA base kernel VA (= the basepage VA on the kernel side).
    // SAFETY: BDOS lock held.
    let env_phys = unsafe {
        alloc_env_for(env, tpa.base_kva as *mut c_void, env_owned_by_child)?
    };

    // Write the BasePage into the reserved window at the TPA start.
    populate_basepage(tpa_bytes, img, tail, env_phys);

    // Populate the classic TOS low-memory system area (sysvars + OSHEADER +
    // cookie jar) in the reserved low region, per-process (B1 / Option 1).
    // A 1:1-ported TOS app reads `_sysbase`@0x4F2 / `_p_cookies`@0x5A0 / the
    // OSHEADER directly (e.g. TeraDesk `main.c:56`), so these must be a real,
    // populated instance in the app's own EL0-VA space. Ordered before the
    // cache maintenance below so its `dsb ish` makes the writes observable to
    // EL0 before the process runs.
    populate_sysvars(tpa_bytes);

    // Write the per-process EL0 upcall trampoline into the reserved
    // low-memory gap (cooperative-kernel building block 1,
    // docs/analysis/coop-kernel/10-upcall-signal-design.md §4). Same page-0
    // region, same pre-cache-maintenance ordering as the sysvars, so the
    // `sync_code_cache` below makes it observable to the EL0 fetch.
    write_upcall_trampoline(tpa_bytes);

    // Write the AES `G_USERDEF` callout stub (AES-as-thread design §4) into the
    // same reserved low gap, directly above the resume trampoline. Written in
    // the same window/ordering so the `sync_code_cache` below covers it too.
    write_userdef_trampoline(tpa_bytes);

    // Make the freshly-written program bytes observable to instruction
    // fetch on real hardware (clean D-cache to PoU + invalidate I-cache
    // over the image region only). QEMU has no caches, so this is a
    // no-op there. Cortex-A72 caches are PIPT, so by-VA maintenance via
    // the kernel alias hits the same PA the program later fetches.
    // SAFETY: `[base_kva, base_kva + image_top)` is live and mapped.
    unsafe {
        mmu::sync_code_cache(tpa.base_kva as usize, image_top as usize);
    }

    // Install the per-process page tables and map every TPA page into
    // the slot's low half at `USER_VA_BASE + off`. All pages are mapped
    // `UserRwExec` (image + heap + stack share the attribute; a per-
    // segment RX/RW split is a later hardening step).
    // SAFETY: slot bounds-checked; single-CPU under the lock.
    unsafe {
        mmu::init_slot_tables(slot)?;
        let page_size = arena::page_size();
        let mut off = 0u64;
        while off < arena::TPA_SIZE {
            mmu::map_page(
                slot,
                USER_VA_BASE + off,
                tpa.base_pa + off,
                PageFlags::UserRwExec,
            )?;
            off += page_size;
        }
    }

    // Register the TPA as the first per-process EL0 user-memory block
    // (ADR-021): an EL0 `Mshrink`/`Mfree` of the basepage arrives as user
    // VA 0, which must translate to the TPA's pool block (`raw_start`)
    // instead of failing `EIMBA`. This also resets the slot's heap-window
    // table so a reused slot starts clean.
    // SAFETY: BDOS lock held; slot bounds-checked.
    unsafe {
        super::usermem::seed_tpa(slot, tpa.raw_start, tpa.base_pa, arena::TPA_SIZE);
    }

    // Publish the slot metadata so `Pterm` (via `slot_for(run)`) and the
    // trap handler can find this process. `basepage_kva` is the high-
    // half VA of the TPA base; `basepage_pa` the low-half phys the user
    // maps via TTBR0; `l0_pa` the page-table root the handler installs.
    let l0_pa = mmu::l0_pa_for(slot);
    let proc_slot = &PROCESS_TABLE[slot];
    proc_slot
        .basepage_kva
        .store(tpa.base_kva as *mut BasePage, Ordering::Release);
    proc_slot.basepage_pa.store(tpa.base_pa, Ordering::Release);
    proc_slot.l0_pa.store(l0_pa, Ordering::Release);
    proc_slot.entry_va.store(img.entry_va, Ordering::Release);
    // Record the program ABI (ELF class) so the GEM trap dispatch reads
    // this process's AES/VDI parameter blocks at the caller's pointer
    // width (ADR-020): 4-byte pointers for an ILP32 (ELFCLASS32) app,
    // 8-byte for an LP64 (ELFCLASS64) one.
    proc_slot.set_abi(if img.is_ilp32() {
        super::Abi::Ilp32
    } else {
        super::Abi::Lp64
    });

    Ok(LoadedImage {
        slot,
        entry_va: img.entry_va,
        l0_pa,
        basepage_va: USER_VA_BASE,
        sp_el0: USER_TPA_TOP,
    })
}

/// Write the [`BasePage`] into the reserved window at the start of the
/// TPA (`tpa[0..sizeof(BasePage)]`).
///
/// Sources:
///   * `p_lowtpa` / `p_hitpa` — TPA extents: `[USER_VA_BASE,
///     USER_TPA_TOP)`. `p_hitpa` is the multi-page TPA top, i.e. the
///     initial stack pointer; the heap lives between the BSS end
///     (`p_bbase + p_blen`) and the downward-growing stack.
///   * `p_tbase` / `p_tlen` — first executable `PT_LOAD` segment.
///   * `p_dbase` / `p_dlen` — first non-executable, writable
///     `PT_LOAD` segment, file portion only.
///   * `p_bbase` / `p_blen` — same segment's BSS tail
///     (`mem_size - file_size`).
///   * `p_xdta` — points at `p_cmdlin` per Atari convention; the
///     program may override via `Fsetdta`.
///   * `p_flags` — [`PF_STANDARD`] (TT-RAM-friendly modern flags).
///   * `p_uft` — all `-1` (no inherited handle redirections).
///
/// `p_cmdlin` receives the caller's command `tail` (verbatim, truncated
/// to 127 bytes, NUL-terminated — EmuTOS `init_pd_fields`
/// `bdos/proc.c:378-383`); `p_env` receives the pool address of the
/// copied environment block (`env_phys`), or `0` when none was
/// allocated. `p_parent`, `p_curdir` and the saved-register cells are
/// left zero (parent linkage is set by the launch path; current-dir
/// inheritance is a Phase-08 concern).
fn populate_basepage(
    tpa: &mut [u8],
    img: &ElfImage,
    tail: &[u8],
    env_phys: Option<Long>,
) {
    let mut text_base: u64 = 0;
    let mut text_len: u64 = 0;
    let mut data_base: u64 = 0;
    let mut data_len: u64 = 0;
    let mut bss_base: u64 = 0;
    let mut bss_len: u64 = 0;

    for seg in img.segments() {
        let is_x = (seg.flags & PF_X) != 0;
        let is_w = (seg.flags & PF_W) != 0;
        if is_x && text_base == 0 {
            text_base = seg.vaddr;
            text_len = seg.file_size;
        } else if is_w && !is_x && data_base == 0 {
            data_base = seg.vaddr;
            data_len = seg.file_size;
            bss_base = seg.vaddr + seg.file_size;
            bss_len = seg.mem_size - seg.file_size;
        }
    }

    // Carve the basepage window out of the TPA. Splitting first gives
    // us a non-aliased mutable handle on exactly the 256 bytes the
    // BasePage owns.
    let (bp_bytes, _rest) = tpa.split_at_mut(BASEPAGE_RESERVED as usize);

    // SAFETY: `bp_bytes` is exactly `size_of::<BasePage>()` bytes,
    // page-aligned (and therefore 4-aligned, satisfying
    // `align_of::<BasePage>() == 4`), and we hold the only mutable
    // borrow over it for the lifetime of `bp`.
    let bp = unsafe { &mut *(bp_bytes.as_mut_ptr() as *mut BasePage) };

    bp.p_lowtpa = USER_VA_BASE as Long;
    bp.p_hitpa = USER_TPA_TOP as Long;
    bp.p_tbase = text_base as Long;
    bp.p_tlen = text_len as Long;
    bp.p_dbase = data_base as Long;
    bp.p_dlen = data_len as Long;
    bp.p_bbase = bss_base as Long;
    bp.p_blen = bss_len as Long;
    bp.p_xdta = (USER_VA_BASE
        + core::mem::offset_of!(BasePage, p_cmdlin) as u64) as Long;
    bp.p_parent = 0;
    bp.p_flags = PF_STANDARD;
    bp.p_env = env_phys.unwrap_or(0);
    bp.p_uft = [-1; NUMSTD];
    bp.p_lddrv = 0;
    bp.p_curdrv = 0;

    // Command tail → `p_cmdlin` (offset 0x80). EmuTOS `init_pd_fields`
    // copies the caller's `tail` byte-for-byte up to `PDCLSIZE` or the
    // first NUL, then appends a NUL. We cap the copy at `PDCLSIZE - 1`
    // (127) so the terminating NUL always fits inside the 128-byte
    // buffer (the spec's "127-byte usable region + trailing NUL"); the
    // caller's `tail` carries no embedded NUL (it was read up to the
    // first one), so this is the same observable image. Under the Atari
    // convention `tail[0]` is the length byte, preserved verbatim.
    let n = core::cmp::min(tail.len(), PDCLSIZE - 1);
    bp.p_cmdlin[..n].copy_from_slice(&tail[..n]);
    bp.p_cmdlin[n] = 0;
}

// ── Classic low-memory system area (B1 / Option 1) ──────────────────────
//
// Per-process EL0-VA layout of the reserved low region the loader populates
// so a 1:1-ported TOS app can dereference the classic absolute addresses
// (`_sysbase`@0x4F2, `_p_cookies`@0x5A0, the OSHEADER). Every value/behaviour
// replicates EmuTOS 1:1 (`references/emutos/...`), cited at each write.
// Spec: `docs/analysis/bios-audit/b1-lowmem-feasibility.md` §Option 1,
// `docs/api/bios/_boot-init.md`, `docs/api/structures/osheader.md`.

/// OSHEADER instance base. `sysbase`@0x4F2 points here. The struct is 48
/// bytes (`OSHEADER` = 0x30, `docs/api/structures/osheader.md`), so it spans
/// `[0x5C0, 0x5F0)`, ending exactly at the cookie jar.
const OSHEADER_VA: u32 = 0x5C0;

/// Cookie-jar base. `p_cookies`@0x5A0 points here.
const COOKIE_JAR_VA: u32 = 0x5F0;

/// Cookie-jar capacity in `{tag, value}` slots (8 bytes each). The region
/// `[0x5F0, 0x670)` holds 16 slots and stays below [`RESERVED_LOW_END`].
const COOKIE_JAR_SLOTS: u32 = 16;

/// Exclusive end of the classic low-memory system area (= jar end, 0x670).
const SYSVAR_AREA_END: u64 = COOKIE_JAR_VA as u64 + (COOKIE_JAR_SLOTS as u64) * 8;

/// EL0 VA of the per-process upcall trampoline (cooperative-kernel building
/// block 1, `docs/analysis/coop-kernel/10-upcall-signal-design.md` §4.1).
/// Placed in the reserved low-memory gap `[SYSVAR_AREA_END (0x670),
/// RESERVED_LOW_END (0x1000))`, inside page 0 (mapped [`PageFlags::UserRwExec`]),
/// so it is EL0-executable with no extra mapping and collides with nothing
/// (above the sysvar area, below the app image base). The kernel-side model
/// of FreeMiNT's per-process `user_things` trampoline (`user_things.S`), at a
/// fixed VA rather than via `tp_ptr`.
pub const TRAMPOLINE_VA: u64 = 0x700;

// The trampoline (5 × 4-byte A64 instructions) must sit wholly in the gap
// between the sysvar area and the app image base.
const _: () = assert!(TRAMPOLINE_VA >= SYSVAR_AREA_END);
const _: () = assert!(TRAMPOLINE_VA + 5 * 4 <= RESERVED_LOW_END);

/// EL0 VA of the per-process **AES `G_USERDEF` callout stub** (AES-as-thread
/// design, `docs/analysis/coop-kernel/30-aes-thread-design.md` §4). Placed in
/// the same reserved low-memory gap as [`TRAMPOLINE_VA`], directly above it
/// (`[0x714, 0x730)`, 7 × 4-byte A64 instructions). The kernel delivers this VA
/// as an upcall handler with `x0 = ub_code`, `x1 = parmblk_ptr`; the stub calls
/// `ub_code(parmblk_ptr)` at EL0 (user→user, no PXN), stores its WORD result in
/// [`USERDEF_RET_SLOT_VA`], then issues `Psigreturn` exactly like the resume
/// trampoline. `ub_code`/`parmblk_ptr` travel as upcall **arguments**, so the
/// stub is written once at load (like [`write_upcall_trampoline`]), never
/// re-emitted per call.
pub const USERDEF_TRAMPOLINE_VA: u64 = 0x714;

/// EL0 VA of the **PARMBLK scratch region** the kernel writes before an AES
/// `G_USERDEF` callout and the app's `ub_code` reads (design §4). ABI-wide: a
/// LP64 `PARMBLK` is 40 bytes, an ILP32 one 32 bytes — both fit the reserved
/// `[0x730, 0x760)` window (48 bytes). Written through the kernel high-half
/// alias (`basepage_kva + offset`), read by EL0 at this low-half VA.
pub const PARMBLK_SCRATCH_VA: u64 = 0x730;

/// EL0 VA of the **userdef return-WORD slot** (design §4). The callout stub
/// stores `ub_code`'s WORD result here (`str w0, [#0x760]`); `Psigreturn` reads
/// it through the kernel high-half alias to feed `aes::thread::resume`. The
/// value is baked into the stub as a `mov` immediate, so it must stay `0x760`.
pub const USERDEF_RET_SLOT_VA: u64 = 0x760;

// The userdef stub (7 × 4-byte A64 instructions = 28 B) sits between the resume
// trampoline and the PARMBLK scratch; the scratch + ret-slot stay inside the
// reserved low gap, below the app image base.
const _: () = assert!(USERDEF_TRAMPOLINE_VA >= TRAMPOLINE_VA + 5 * 4);
const _: () = assert!(USERDEF_TRAMPOLINE_VA + 7 * 4 <= PARMBLK_SCRATCH_VA);
// The widest PARMBLK the writer emits is the LP64 layout (40 bytes); it must
// fit before the ret-slot.
const _: () = assert!(PARMBLK_SCRATCH_VA + 40 <= USERDEF_RET_SLOT_VA);
const _: () = assert!(USERDEF_RET_SLOT_VA + 4 <= RESERVED_LOW_END);
// The ret-slot VA is a `mov` immediate baked into `USERDEF_STUB_CODE`; keep the
// constant and the encoding in lockstep.
const _: () = assert!(USERDEF_RET_SLOT_VA == 0x760);

/// EL0 VA of the **`Keytbl` area**: the `struct keytbl` XBIOS `Keytbl` (0x10)
/// returns, followed by the three 128-byte scancode→ASCII tables and the three
/// AltGr override arrays it points at. Placed at the top of the same reserved
/// low-memory gap as [`TRAMPOLINE_VA`] / [`PARMBLK_SCRATCH_VA`], above the
/// userdef return slot. Written through the kernel high-half alias
/// (`basepage_kva + offset`) when `Keytbl` is called by a running EL0 process,
/// and read back by that process at this low-half VA — EmuTOS' desktop
/// dereferences the returned pointer directly (`desk/deskmain.c:1144-1145`).
/// See `crate::xbios::keytbl` for the layout inside the area.
pub const KEYTBL_AREA_VA: u64 = 0x768;

/// Size of the [`KEYTBL_AREA_VA`] region, in bytes.
pub const KEYTBL_AREA_SIZE: u64 = 0x238;

// The Keytbl area sits above the userdef return slot and stays inside the
// reserved low gap, below the app image base.
const _: () = assert!(KEYTBL_AREA_VA >= USERDEF_RET_SLOT_VA + 4);
const _: () = assert!(KEYTBL_AREA_VA + KEYTBL_AREA_SIZE <= RESERVED_LOW_END);

/// OSHEADER `os_version` (+0x02): Falcon TOS 4.04. EmuTOS' own default is
/// 0x206 (`config.h:1367-1372`); NewTOS reports 4.04 — user-pinned.
const OS_VERSION: u16 = 0x0404;
/// OSHEADER `os_conf` (+0x1C): `(country << 1) | PAL` (`osheader.md`
/// §os_conf). NewTOS is US/English (`i18n::current_lang == En`, cookie
/// `_AKP == US`) and ignores PAL → country US(0), PAL 0 → 0.
const OS_CONF: u16 = 0x0000;
/// OSHEADER `os_date` (+0x18): OS build date, BCD `MMDDYYYY`. NewTOS pins no
/// build-date constant; placeholder = the codebase's default epoch
/// 1980-01-01 (`tos-api/src/gemdos/time.rs:26`). Flagged to the lead.
const OS_DATE: u32 = 0x0101_1980;
/// OSHEADER `os_dosdate` (+0x1E): same date, FAT-packed
/// (`(year-1980)<<9 | month<<5 | day`) → 1980-01-01.
const OS_DOSDATE: u16 = 0x0021;

/// RAM-config validity magics (`references/emutos/bios/memory.S`), written on
/// cold boot at the end of `memconf` (memory.S:588-590).
const MEMVALID_MAGIC: u32 = 0x7520_19F3; // $420 memvalid (memory.S:126,588)
const MEMVAL2_MAGIC: u32 = 0x2376_98AA; //  $43A memval2  (memory.S:129,589)
const MEMVAL3_MAGIC: u32 = 0x5555_AAAA; //  $51A memval3  (memory.S:132,590)

/// `conterm` ($484, BYTE) boot default: keyclick + key-repeat + system-bell
/// enabled (`references/emutos/bios/ikbd.c:1100` — `conterm = 7`). Bit 3
/// (`& 8`) requests the `Bconin` keyboard-shift-state fold
/// (`bios/ikbd.c:279-280`); the default leaves it clear, so `Bconin` keeps
/// returning bits 24-31 = 0 exactly as before this sysvar was populated.
const CONTERM_DEFAULT: u8 = 7;

/// Write a 32-bit little-endian `LONG` at byte offset `off` (any alignment;
/// the classic sysvars are 4-byte and not all 4-aligned, e.g. `sysbase`@0x4F2).
#[inline]
fn wr_long(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
/// Write a 16-bit little-endian `WORD` at byte offset `off` (any alignment).
#[inline]
fn wr_word(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

/// Populate the classic TOS low-memory system area in the reserved low
/// region of a freshly-loaded TPA (B1 / Option 1).
///
/// Writes, per-process and through the kernel high-half alias `tpa`:
///   * an **OSHEADER instance** at [`OSHEADER_VA`]
///     (`docs/api/structures/osheader.md`),
///   * the **classic system variables** at their fixed `$400`-region
///     addresses (`docs/api/bios/_boot-init.md` §7, EmuTOS `tosvars.ld`),
///   * a **cookie jar** at [`COOKIE_JAR_VA`], mirroring the kernel jar
///     ([`crate::cookie`]) in the classic `{tag, value}` array form
///     (`docs/api/bios/_cookie-jar.md`) so `_p_cookies`@0x5A0 reaches a
///     well-formed jar.
///
/// `tpa` is the whole-TPA byte view; every write lands in the first page,
/// disjoint from the BasePage (`[0x000, 0x100)`) and the app image
/// (`>= 0x1000`). Called before the code-cache maintenance in
/// [`populate_and_map`] so its `dsb ish` makes the writes observable to EL0
/// before the process runs.
fn populate_sysvars(tpa: &mut [u8]) {
    // ── OSHEADER instance (osheader.md; EmuTOS bios/startup.S:73-114
    //    `_os_header`) ─────────────────────────────────────────────────
    let oh = OSHEADER_VA as usize;
    // os_entry (+0x00): m68k `BRA.S reseth` reset-vector opcode — no m68k
    // reset path on AArch64 (osheader.md §os_entry). Left 0.
    // os_version (+0x02):
    wr_word(tpa, oh + 0x02, OS_VERSION);
    // reseth (+0x04): m68k C reset handler — none on AArch64. Left 0.
    // os_beg (+0x08) / os_end (+0x0C): EmuTOS = OS-image self-pointer +
    // end-of-OS-ST-RAM (startup.S:88-91). NewTOS reports the current usable
    // ST-RAM pool bounds here per the BB1 task decision (the STRAM/TTRAM
    // 8 MB split is BB2); both fit an Atari LONG (low-2 GiB pool).
    let (stram_start, stram_end) = crate::bdos::mem::init::stram_range();
    wr_long(tpa, oh + 0x08, stram_start as u32);
    wr_long(tpa, oh + 0x0C, stram_end as u32);
    // os_rsvl (+0x10): reserved; EmuTOS sets 0 (osheader.md §os_rsvl). 0.
    // os_magic (+0x14): pointer to the GEM_MUPB — populated only once GEM/AES
    // is reimplemented into a per-process GEM_MUPB (osheader.md §os_magic);
    // its `gm_init` (AES entry) has no faithful native EL0-VA value yet, so
    // it is left 0 rather than guessed (flagged to the lead). 0.
    // os_date (+0x18):
    wr_long(tpa, oh + 0x18, OS_DATE);
    // os_conf (+0x1C):
    wr_word(tpa, oh + 0x1C, OS_CONF);
    // os_dosdate (+0x1E):
    wr_word(tpa, oh + 0x1E, OS_DOSDATE);
    // os_root (+0x20): BDOS quick-pool head (EL1-only). Left 0.
    // os_kbshift (+0x24): ptr to the kbshift byte (EL1-only). Left 0.
    // os_run (+0x28): `PD**` → the cell holding the current-PD ptr (EmuTOS
    // startup.S:110-111, `_boot-init.md` §4). Under NewTOS Option-1
    // single-tasking the running PD is the BasePage at USER_VA_BASE (0), and
    // BasePage[0] (`p_lowtpa`) == 0 == that same VA, so pointing os_run at
    // the BasePage yields the classic `*(os_run) == current-PD` contract.
    wr_long(tpa, oh + 0x28, USER_VA_BASE as u32);
    // os_dummy (+0x2C): OS signature. EmuTOS 'ETOS'; NewTOS 'NTOS'
    // (osheader.md §os_dummy).
    wr_long(tpa, oh + 0x2C, u32::from_be_bytes(*b"NTOS"));

    // ── Classic system variables ($400 region, _boot-init.md §7; EmuTOS
    //    tosvars.ld + memory.S) ─────────────────────────────────────────
    // memvalid/memval2/memval3: RAM-config validity magics (cold-boot sets
    // them at the end of memconf).
    wr_long(tpa, 0x420, MEMVALID_MAGIC);
    wr_long(tpa, 0x43A, MEMVAL2_MAGIC);
    wr_long(tpa, 0x51A, MEMVAL3_MAGIC);
    // resvalid/resvector ($426/$42A): warm-reset vector pair — EmuTOS never
    // sets them (cold-boot value 0, _boot-init.md §5); an app pokes them.
    wr_long(tpa, 0x426, 0);
    wr_long(tpa, 0x42A, 0);
    // phystop ($42E) = physical top of ST-RAM; membot ($432) = start of TPA;
    // memtop ($436) = end of TPA (biosmem.c:66-72). NewTOS: the ST-RAM pool
    // bounds (no video-RAM carve-out, so phystop == memtop == pool end).
    wr_long(tpa, 0x42E, stram_end as u32);
    wr_long(tpa, 0x432, stram_start as u32);
    wr_long(tpa, 0x436, stram_end as u32);
    // bootdev ($446): default boot drive (bios.c:798-799). NewTOS boots from
    // the first available hard disk (C:), i.e. the lowest set bit in the
    // drive map (drive.rs FIRST_HD == C: == 2).
    let drvbits = crate::bios::drive::drvbits();
    let bootdev = if drvbits != 0 {
        drvbits.trailing_zeros() as u16
    } else {
        crate::bdos::fs::drive::current_drive()
    };
    wr_word(tpa, 0x446, bootdev);
    // cmdload ($482): load COMMAND.PRG instead of the ROM shell when != 0;
    // boot-config flag, default 0 (ROM-shell path, EmuTOS `bios.c:850` gate,
    // `_boot-init.md` §7c). NewTOS has no COMMAND.PRG override, so 0.
    wr_word(tpa, 0x482, 0);
    // conterm ($484, BYTE): console attribute bits, boot default 7 = keyclick
    // + key-repeat + system-bell on (EmuTOS `bios/ikbd.c:1100`). Bit 3
    // (`& 8`) is the `Bconin` keyboard-shift-state fold request
    // (`bios/ikbd.c:279`); read per-process by `pack_console_key`
    // (`bios/chardev.rs`). Single byte — the surrounding TPA is already
    // zeroed, so the $485 alignment gap stays 0.
    tpa[0x484] = CONTERM_DEFAULT;
    // dumpflg ($4EE): hardcopy/screen-dump-in-progress flag, boot default -1
    // (EmuTOS `bios/bios.c:357` — `dumpflg = -1;`); consumed by `Scrdmp`
    // (XBIOS, out of scope). `-1` as a WORD is 0xFFFF.
    wr_word(tpa, 0x4EE, (-1i16) as u16);
    // _hz_200 ($4BA): 200 Hz system-tick counter. Static snapshot at load
    // (monotonic ms / 5); the live low-memory writer is a documented
    // deferral (feasibility §3) — TeraDesk does not poll it live.
    let hz200 = crate::host::host()
        .map(|h| (h.monotonic_millis() / 5) as u32)
        .unwrap_or(0);
    wr_long(tpa, 0x4BA, hz200);
    // drvbits ($4C2): physical drive bitmap (Drvmap, blkdev_init).
    wr_long(tpa, 0x4C2, drvbits);
    // sysbase ($4F2): pointer to the OSHEADER (bios.c:358) — the field whose
    // absence crashed TeraDesk (`*(long*)0x4F2`). Adjacent shell_p@0x4F6 is
    // left 0, so an LP64 8-byte read of $4F2 still yields OSHEADER_VA.
    wr_long(tpa, 0x4F2, OSHEADER_VA);
    // p_cookies ($5A0): pointer to the cookie jar (cookie_init, util/cookie.c).
    wr_long(tpa, 0x5A0, COOKIE_JAR_VA);
    // prt_stat ($506) / prt_vec ($50A): printer status/output redirection
    // vectors. EmuTOS points them at `just_rts` (a no-op RTS stub) in
    // `chardev_init` (`_boot-init.md` §7c). NewTOS has no printer device and
    // no m68k vector page to install, so they stay 0 (no redirection =
    // printer absent) — HW-absence, matching the OSHEADER's m68k-only
    // os_entry/reseth fields left 0 above.
    wr_long(tpa, 0x506, 0);
    wr_long(tpa, 0x50A, 0);

    // ── Cookie jar (classic array form, _cookie-jar.md; EmuTOS
    //    util/cookie.c:26-35) ───────────────────────────────────────────
    // Mirror the kernel jar (crate::cookie) as `{tag, value}` LONG pairs,
    // terminated with `{0, capacity}`. Native apps normally use the Ssystem
    // cookie subset (ADR-012); this is the direct-$5A0 compatibility path.
    let jar = COOKIE_JAR_VA as usize;
    let mut slot = 0u32;
    let n = crate::cookie::count();
    for i in 1..=n {
        if slot >= COOKIE_JAR_SLOTS - 1 {
            break; // leave the last slot for the terminator
        }
        if let Some(tag) = crate::cookie::slot_tag(i) {
            let value = crate::cookie::get(tag).unwrap_or(0);
            let off = jar + (slot as usize) * 8;
            wr_long(tpa, off, tag);
            wr_long(tpa, off + 4, value);
            slot += 1;
        }
    }
    // Terminator {tag = 0, value = capacity} (EmuTOS cookie_init).
    let off = jar + (slot as usize) * 8;
    wr_long(tpa, off, 0);
    wr_long(tpa, off + 4, COOKIE_JAR_SLOTS);
}

/// The five AArch64 instructions of the EL0 upcall trampoline (design §4.2),
/// stored as little-endian words. On handler `ret` (PC = LR =
/// [`TRAMPOLINE_VA`]) it calls `Psigreturn` (GEMDOS 0x11A) — bit-exact with
/// FreeMiNT's `move.w #0x011a` (`user_things.S`) — to resume the interrupted
/// EL0 context; the tail is FreeMiNT's `pc_valid_return` safety net
/// (`Pterm(-998)`), taken only if `Psigreturn` ever wrongly returns. No EL0
/// stack push is needed: the handler argument and return address travel in
/// registers (x0..x3, LR), not on the stack, per AAPCS64 (design §4.2).
///
/// Encodings verified with `aarch64-elf-as`:
/// ```text
///   d2802348  mov x8, #0x11A   ; Psigreturn function id
///   d4000021  svc #1           ; GEMDOS trap — does not return
///   d29f8340  mov x0, #0xFC1A  ; -998 as UWORD exit code
///   d2800988  mov x8, #0x4C    ; Pterm
///   d4000021  svc #1
/// ```
const TRAMPOLINE_CODE: [u32; 5] =
    [0xD280_2348, 0xD400_0021, 0xD29F_8340, 0xD280_0988, 0xD400_0021];

/// Write the EL0 upcall trampoline (design §4.2) into the reserved
/// low-memory gap at [`TRAMPOLINE_VA`], through the kernel high-half alias
/// `tpa`. Called from [`populate_and_map`] alongside [`populate_sysvars`],
/// before the code-cache maintenance, so the freshly written instructions
/// are cleaned to PoU / I-cache-invalidated over the image window
/// (`image_top >= RESERVED_LOW_END > TRAMPOLINE_VA + 20`) and are visible to
/// the EL0 instruction fetch on real hardware (QEMU is cache-less).
fn write_upcall_trampoline(tpa: &mut [u8]) {
    let base = TRAMPOLINE_VA as usize;
    for (i, insn) in TRAMPOLINE_CODE.iter().enumerate() {
        let off = base + i * 4;
        tpa[off..off + 4].copy_from_slice(&insn.to_le_bytes());
    }
}

/// The seven AArch64 instructions of the AES `G_USERDEF` callout stub
/// (AES-as-thread design §4). Delivered as an EL0 upcall with `x0 = ub_code`,
/// `x1 = parmblk_ptr` (AAPCS64); it calls `ub_code(parmblk_ptr)` at EL0
/// (user→user — no PXN, exactly XaAES' `progdef_callout`), stores the WORD
/// result in the ret-slot, then issues `Psigreturn` (GEMDOS 0x11A) so the kernel
/// resumes the stalled AES fiber.
///
/// Encodings verified with `aarch64-elf-as`:
/// ```text
///   aa0003e9  mov x9, x0        ; x9 = ub_code
///   aa0103e0  mov x0, x1        ; x0 = parmblk_ptr (arg0 to ub_code)
///   d63f0120  blr x9            ; ub_code(parmblk_ptr) -> WORD in w0
///   d280ec09  mov x9, #0x760    ; USERDEF_RET_SLOT_VA
///   b9000120  str w0, [x9]      ; store the userdef WORD result
///   d2802348  mov x8, #0x11A    ; Psigreturn
///   d4000021  svc #1            ; GEMDOS trap — resumes the AES fiber
/// ```
const USERDEF_STUB_CODE: [u32; 7] = [
    0xAA00_03E9, 0xAA01_03E0, 0xD63F_0120, 0xD280_EC09, 0xB900_0120, 0xD280_2348,
    0xD400_0021,
];

/// Write the AES `G_USERDEF` callout stub (design §4) into the reserved
/// low-memory gap at [`USERDEF_TRAMPOLINE_VA`], through the kernel high-half
/// alias `tpa`. Called from [`populate_and_map`] alongside
/// [`write_upcall_trampoline`], **before** the code-cache maintenance, so the
/// stub is cleaned to PoU / I-cache-invalidated over the image window
/// (`image_top >= RESERVED_LOW_END > USERDEF_TRAMPOLINE_VA + 28`) and is visible
/// to the EL0 instruction fetch on real hardware (QEMU is cache-less). Like the
/// resume trampoline, this is written once at load — `ub_code`/`parmblk_ptr`
/// arrive as upcall arguments, not baked constants.
fn write_userdef_trampoline(tpa: &mut [u8]) {
    let base = USERDEF_TRAMPOLINE_VA as usize;
    for (i, insn) in USERDEF_STUB_CODE.iter().enumerate() {
        let off = base + i * 4;
        tpa[off..off + 4].copy_from_slice(&insn.to_le_bytes());
    }
}

/// Resolve an [`EnvSpec`] into an optional pool address for `p_env`,
/// allocating + copying the environment via the BDOS pool. Mirrors
/// EmuTOS `alloc_env` (`bdos/proc.c:413-431`) plus the `set_owner`
/// hand-off (`bdos/proc.c:305-307`).
///
/// `child_cookie` is the child basepage's kernel VA (the ownership tag
/// `free_all_owned` matches); `owned_by_child` selects whether the env
/// block is re-tagged to the child (`PE_LOADGO`) or left owned by the
/// allocating process (`PE_LOAD`).
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
unsafe fn alloc_env_for(
    env: EnvSpec<'_>,
    child_cookie: *mut c_void,
    owned_by_child: bool,
) -> Result<Option<Long>, LoadError> {
    // Materialise the source env image (including its terminating
    // double-NUL) as a byte slice, then copy it into a fresh pool block.
    match env {
        EnvSpec::None => Ok(None),
        EnvSpec::Explicit(img) => {
            // SAFETY: BDOS lock held.
            let phys = unsafe { alloc_env(img, child_cookie, owned_by_child)? };
            Ok(Some(phys))
        }
        EnvSpec::Inherit => {
            // env == NULL → duplicate run->p_env (EmuTOS proc.c:419-420).
            let run = crate::bdos::process::current();
            // SAFETY: BDOS lock held; `run`/its `p_env` phys point at
            // live pool memory when non-null.
            let phys = unsafe {
                let src: &[u8] = if run.is_null() {
                    &EMPTY_ENV
                } else {
                    let penv = (*run).p_env;
                    if penv == 0 {
                        &EMPTY_ENV
                    } else {
                        env_slice_from_phys(penv)
                    }
                };
                alloc_env(src, child_cookie, owned_by_child)?
            };
            Ok(Some(phys))
        }
    }
}

/// Allocate a pool block for `img` and copy it verbatim, returning the
/// block's pool address. `img` is the env content **including** its
/// terminating double-NUL (its length is EmuTOS's `envsize`). The size
/// is rounded up to even (`bdos/proc.c:421`) and the rounding tail is
/// zero-filled.
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
unsafe fn alloc_env(
    img: &[u8],
    child_cookie: *mut c_void,
    owned_by_child: bool,
) -> Result<Long, LoadError> {
    // Round the byte count up to even; never below the 2-byte empty env.
    let size = (((img.len() + 1) & !1).max(2)) as Long;
    // PF_STANDARD carries PF_TTRAMLOAD, so `alloc_env`'s TT-RAM-preferred
    // arm applies (`(flags & PF_TTRAMLOAD) ? MX_PREFTTRAM : MX_STRAM`).
    // SAFETY: BDOS lock held.
    let phys = unsafe { crate::bdos::mem::traps::mxalloc(size, MX_PREFTTRAM) };
    if phys == 0 {
        return Err(LoadError::NoEnvMemory);
    }
    // Copy verbatim into the pool block (reachable at phys + VA_OFFSET),
    // zero-padding the even-rounding tail.
    let dst = pa_to_kva(phys as u64) as *mut u8;
    // SAFETY: `dst` aliases `size` bytes of live, RW-mapped pool memory
    // (whole DRAM is linearly mapped in the high half); `img` is a
    // distinct, live slice.
    unsafe {
        core::ptr::copy_nonoverlapping(img.as_ptr(), dst, img.len());
        for i in img.len()..size as usize {
            dst.add(i).write(0);
        }
    }
    // PE_LOADGO: hand ownership to the child so its `Pterm` frees it.
    if owned_by_child {
        // SAFETY: BDOS lock held; `phys` is on the allocated list.
        unsafe {
            crate::bdos::mem::traps::set_owner(phys, child_cookie);
        }
    }
    Ok(phys)
}

/// Borrow the environment block living at pool physical address `penv`
/// as a byte slice, including its terminating double-NUL. Scans for the
/// `\0\0` terminator, bounded by [`ENV_SCAN_LIMIT`] against a malformed
/// (unterminated) parent env.
///
/// # Safety
///
/// Caller holds the BDOS giant lock and `penv` addresses a live pool
/// environment block (invariant of every `p_env` this loader writes).
unsafe fn env_slice_from_phys(penv: Long) -> &'static [u8] {
    let base = pa_to_kva(penv as u64) as *const u8;
    let mut n = 0usize;
    while n + 1 < ENV_SCAN_LIMIT {
        // SAFETY: `n + 1 < ENV_SCAN_LIMIT`; pool memory is mapped.
        let b0 = unsafe { base.add(n).read() };
        let b1 = unsafe { base.add(n + 1).read() };
        if b0 == 0 && b1 == 0 {
            n += 2; // include the terminating double-NUL
            break;
        }
        n += 1;
    }
    // SAFETY: `base .. base+n` is within the scanned, mapped range.
    unsafe { core::slice::from_raw_parts(base, n) }
}

/// Upper bound on a parent environment scan (`env_slice_from_phys`),
/// guarding against an unterminated block. Generous versus real Atari
/// environments (a handful of `KEY=value` records).
const ENV_SCAN_LIMIT: usize = 4096;

/// Reject segments that clobber the reserved low region or overrun the
/// usable TPA image region.
///
/// A `PT_LOAD` may now span many pages; it must start at or after the
/// reserved low region ([`RESERVED_LOW_END`], the first page above the
/// BasePage + classic sysvars/OSHEADER/cookie-jar area — B1/Option 1) and
/// end at or before `USER_TPA_TOP - USER_STACK_MIN`, leaving the top of the
/// TPA for the downward-growing stack. The heap occupies whatever is left
/// between the image top and the stack.
fn validate_segment(seg: &LoadSegment) -> Result<(), LoadError> {
    // VA must lie within [RESERVED_LOW_END, image_top). Segments below the
    // reserved low region would overlap the per-process sysvar/OSHEADER area
    // `populate_sysvars` writes (sysbase@0x4F2 etc.), so they are rejected —
    // the SDK links native apps at RESERVED_LOW_END (`sdk/newtos.ld`).
    let min_va = USER_VA_BASE + RESERVED_LOW_END;
    if seg.vaddr < min_va {
        return Err(LoadError::SegmentVaOutOfRange);
    }
    let end = seg
        .vaddr
        .checked_add(seg.mem_size)
        .ok_or(LoadError::ProgramTooLarge)?;
    if end > USER_TPA_TOP - USER_STACK_MIN {
        return Err(LoadError::ProgramTooLarge);
    }
    // PF_R is implicit for any LOAD; PF_W and PF_X drive the text/data
    // classification in `populate_basepage`. Every TPA page is mapped
    // UserRwExec, so the flags don't gate the mapping decision yet — a
    // per-segment RX/RW split is a later hardening step.
    let _ = seg.flags & (PF_W | PF_X);
    Ok(())
}
