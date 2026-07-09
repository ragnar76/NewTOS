//! ELF-to-slot loader (Phase 06 D3).
//!
//! Combines [`super::elf`] (parser), [`super::arena`] (per-slot user
//! page) and [`super::mmu`] (per-slot page tables) into a single
//! function: given an ELF blob and a process slot, copy the program
//! into the slot's user page and install the low-half mapping that
//! lets EL0 fetch from it.
//!
//! ## Phase-06-D3 limitations
//!
//! - **One 4-KiB page total.** Programs larger than 4 KiB
//!   (text + data + bss combined) currently fail with
//!   [`LoadError::ProgramTooLarge`]. Lifting this requires
//!   extending [`super::mmu`] to install multiple `L3` entries and
//!   [`super::arena`] to hand out multiple pages per slot.
//! - **No segment-permission split.** All segments share the single
//!   page mapped [`super::mmu::PageFlags::UserRwExec`] (combined R+W
//!   +X). Real text/data/bss separation arrives when the
//!   multi-page mapping does.
//! - **Static load address.** Every PT_LOAD segment must have
//!   `p_vaddr` inside
//!   `[USER_VA_BASE + sizeof(BasePage), USER_VA_BASE + 4 KiB)`.
//!   The first 256 bytes of the page are reserved for the
//!   [`BasePage`] that [`populate_basepage`] writes after the
//!   segment copy. Anything below the reserved window returns
//!   [`LoadError::SegmentVaOutOfRange`]; anything past the page
//!   end returns [`LoadError::ProgramTooLarge`].
//!
//! These constraints are deliberately tight so the D3 selftest can
//! exercise the **whole pipeline** (parse → copy → map → eret to
//! EL0 → SVC trap-back) without any stub layers. Phase 09 will
//! relax all three when source-port programs land.

use core::ffi::c_void;
use core::sync::atomic::Ordering;

use tos_types::{BasePage, Long, MX_PREFTTRAM, NUMSTD, PDCLSIZE, PF_STANDARD};

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
    /// A `PT_LOAD` segment lies outside the writable region of the
    /// single 4-KiB user page. The first 256 bytes of the page are
    /// reserved for the basepage; segments must therefore start at
    /// `USER_VA_BASE + sizeof(BasePage)` or later. Anything below
    /// the basepage window produces this error.
    SegmentVaOutOfRange,
    /// `mem_size` exceeds the per-slot user page size (4 KiB).
    /// Programs larger than that are out of scope for D3.
    ProgramTooLarge,
    /// `super::mmu::map_page` failed (validation error inside the
    /// page-table builder).
    MapPage(MapError),
    /// The environment block could not be allocated from the BDOS pool
    /// (`Mxalloc` returned 0). EmuTOS `alloc_env` failure →
    /// `ENSMEM` (`bdos/proc.c:284-288`).
    NoEnvMemory,
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
    /// [`populate_basepage`]. Always equals `USER_VA_BASE` for the
    /// D3 single-page layout; multi-page layouts will lift the
    /// basepage to its own page so the user code can be mapped
    /// `RX` while the basepage stays `RW`.
    pub basepage_va: u64,
}

/// Base of the user-space VA window — the only address `map_page`
/// will accept for D3 (single-page mapping). Also the basepage VA
/// (every process's TPA starts here in the per-slot model).
pub const USER_VA_BASE: u64 = 0x0;

/// Number of bytes at the start of the user page reserved for the
/// [`BasePage`]. ELF `PT_LOAD` segments must place themselves at or
/// after `USER_VA_BASE + BASEPAGE_RESERVED`.
const BASEPAGE_RESERVED: u64 = core::mem::size_of::<BasePage>() as u64;

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
    if slot >= super::MAX_PROCESSES {
        return Err(LoadError::InvalidSlot);
    }

    // 1. Parse.
    let img: ElfImage = elf::parse(elf_bytes)?;

    // 2. Validate + copy each PT_LOAD into the slot's user page.
    let page_size = arena::page_size();
    let user_va_end = USER_VA_BASE + page_size;
    // SAFETY: slot has been bounds-checked.
    let page = unsafe { arena::page_bytes_mut(slot) }
        .ok_or(LoadError::InvalidSlot)?;
    // Zero the page first so any unmapped offsets read as zero
    // (BSS-like behaviour for offsets between segments).
    for byte in page.iter_mut() {
        *byte = 0;
    }

    for seg in img.segments() {
        validate_segment(seg, USER_VA_BASE, user_va_end)?;

        let dest_off = (seg.vaddr - USER_VA_BASE) as usize;
        let file_size = seg.file_size as usize;
        let mem_size = seg.mem_size as usize;

        // Copy the file image.
        if file_size > 0 {
            let src = elf_bytes
                .get(seg.file_offset as usize..(seg.file_offset as usize + file_size))
                .ok_or(LoadError::Parse(ParseError::SegmentOutOfRange))?;
            page[dest_off..dest_off + file_size].copy_from_slice(src);
        }
        // Zero the BSS tail. (`page` was zeroed above, but a later
        // segment with a smaller mem_size could have skipped over
        // a region previously zeroed; explicit re-zero here is a
        // belt-and-braces for non-trivial programs that the
        // future multi-segment loader needs to keep doing.)
        if mem_size > file_size {
            let bss_start = dest_off + file_size;
            let bss_end = dest_off + mem_size;
            for b in page[bss_start..bss_end].iter_mut() {
                *b = 0;
            }
        }
    }

    // 3. Allocate + copy the environment (EmuTOS `alloc_env`). Done
    //    before `populate_basepage` so `p_env` can be stamped in one
    //    pass. The child's basepage kernel VA (= the page start) is the
    //    ownership cookie for the `PE_LOADGO` re-tag.
    // SAFETY: BDOS lock held; single-CPU pre-test.
    let env_phys = unsafe {
        alloc_env_for(env, page.as_mut_ptr() as *mut c_void, env_owned_by_child)?
    };

    // 3b. Populate the BasePage in the reserved 256-byte block at
    //    the start of the page. Segments have already been copied
    //    above and `validate_segment` guarantees none of them
    //    overlap the basepage window, so this is the last write to
    //    `page` before the slot mapping is installed.
    populate_basepage(page, &img, tail, env_phys);

    // 3a. The page now holds the final program image. Make those bytes
    //     observable to instruction fetch on real hardware (clean D-cache
    //     to PoU + invalidate I-cache); QEMU has no caches so this is a
    //     no-op there. Done via the kernel VA that owns the page; the
    //     Cortex-A72 PIPT caches make by-VA maintenance hit the same PA the
    //     program later fetches through its EL0 mapping.
    // SAFETY: `page` is a live, mapped kernel slice for this slot.
    unsafe {
        mmu::sync_code_cache(page.as_ptr() as usize, page.len());
    }

    // 4. Install the (single) mapping.
    let user_page_pa = arena::page_phys(slot).ok_or(LoadError::InvalidSlot)?;
    // SAFETY: slot is bounds-checked; pre-test single-CPU.
    unsafe {
        mmu::init_slot_tables(slot)?;
        mmu::map_page(slot, USER_VA_BASE, user_page_pa, PageFlags::UserRwExec)?;
    }

    // 5. Publish the slot metadata so `Pterm` (and the Phase 06 D9
    //    `current_owner` shim) can find this process via
    //    `slot_for(run)`. The basepage_kva is the high-half kernel VA
    //    that owns the page bytes; basepage_pa is the low-half phys
    //    that the user maps via TTBR0; l0_pa is the slot's
    //    page-table root the trap handler installs on context
    //    switch. State stays whatever the caller set — typically
    //    `Loaded`; the caller flips it to `Running` once it has set
    //    `RUN` and is about to `eret`.
    let l0_pa = mmu::l0_pa_for(slot);
    let basepage_kva = page.as_mut_ptr() as *mut BasePage;
    let proc_slot = &PROCESS_TABLE[slot];
    proc_slot.basepage_kva.store(basepage_kva, Ordering::Release);
    proc_slot.basepage_pa.store(user_page_pa, Ordering::Release);
    proc_slot.l0_pa.store(l0_pa, Ordering::Release);
    proc_slot.entry_va.store(img.entry_va, Ordering::Release);

    Ok(LoadedImage {
        slot,
        entry_va: img.entry_va,
        l0_pa,
        basepage_va: USER_VA_BASE,
    })
}

/// Write the [`BasePage`] into the first 256 bytes of `page`.
///
/// Sources:
///   * `p_lowtpa` / `p_hitpa` — TPA extents (= the single user page
///     for D3).
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
    page: &mut [u8; 4096],
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

    // Carve the basepage window out of the page. Splitting first
    // gives us a non-aliased mutable handle on exactly the 256 bytes
    // BasePage owns.
    let (bp_bytes, _rest) = page.split_at_mut(BASEPAGE_RESERVED as usize);

    // SAFETY: `bp_bytes` is exactly `size_of::<BasePage>()` bytes,
    // page-aligned (and therefore 4-aligned, satisfying
    // `align_of::<BasePage>() == 4`), and we hold the only mutable
    // borrow over it for the lifetime of `bp`.
    let bp = unsafe { &mut *(bp_bytes.as_mut_ptr() as *mut BasePage) };

    bp.p_lowtpa = USER_VA_BASE as Long;
    bp.p_hitpa = (USER_VA_BASE + arena::page_size()) as Long;
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

/// Reject segments that don't fit in the single 4-KiB page or that
/// would clobber the basepage window at the start of the page.
fn validate_segment(
    seg: &LoadSegment,
    user_va_base: u64,
    user_va_end: u64,
) -> Result<(), LoadError> {
    // VA must lie within
    // [user_va_base + sizeof(BasePage), user_va_end). Anything below
    // would land in the basepage area we own.
    let min_va = user_va_base + BASEPAGE_RESERVED;
    if seg.vaddr < min_va {
        return Err(LoadError::SegmentVaOutOfRange);
    }
    let end = seg
        .vaddr
        .checked_add(seg.mem_size)
        .ok_or(LoadError::ProgramTooLarge)?;
    if end > user_va_end {
        return Err(LoadError::ProgramTooLarge);
    }
    // PF_R is implicit for any LOAD; PF_W and PF_X drive the
    // text/data classification in `populate_basepage`. For the D3
    // single-page mapping every segment ends up in the same
    // UserRwExec page, so the flags don't gate the mapping decision
    // yet — the multi-page split arrives with the per-segment
    // permission story.
    let _ = seg.flags & (PF_W | PF_X);
    Ok(())
}
