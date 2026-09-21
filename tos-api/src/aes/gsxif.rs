//! The one AES screen-save buffer — EmuTOS `aes/gemgsxif.c`'s `gl_tmp` /
//! `gl_mlen` and the calls around it: `gsx_malloc` (`:108-114`), `gsx_mret`
//! (`:125-129`), `bb_save`/`bb_restore` (`:383-392` → `bb_set`, `:320-380`)
//! — plus the **transient** per-opening block `popup_blit`
//! (`aes/gemmnext.c:136-215`) takes for a pop-up or a nested submenu.
//!
//! EmuTOS keeps exactly **one** buffer for the alert (`fm_alert`,
//! `gemfmalt.c:363,390`) and the drop-down (`menu_sr`, `gemmnlib.c:208-223`),
//! takes it once when the AES goes graphic (`sh_tographic`,
//! `gemshlib.c:236`) from the GEMDOS pool (`dos_alloc_anyram` =
//! `Mxalloc(n, MX_PREFTTRAM)`, `util/gemdos.c:117-120`), and hands its
//! address and length to any program through `wind_get(WF_SCREEN)`. A
//! pop-up or a submenu does **not** use it — the submenu opens *while* the
//! drop-down is down and holds the buffer — but takes a fresh ST-RAM block
//! per opening (`dos_alloc_stram`, `gemmnext.c:155-163`) and frees it on
//! the restore (`:212`).
//!
//! # NewTOS
//!
//! * **Pool.** The buffer is a kernel-owned block of the BDOS ST-/TT-RAM
//!   pool — [`gsx_malloc`] calls the same `Mxalloc(…, MX_PREFTTRAM)` EmuTOS
//!   does, one page over so the page-rounded base is exclusively ours, and
//!   runs before the first process, so the block carries the kernel owner
//!   cookie that every `Pterm` sweep skips (`bdos/mem/traps.rs`,
//!   `KERNEL_PD_SENTINEL`). The kernel reaches it through the high-half
//!   alias `pa + VA_OFFSET`, as `rsrc_load` reaches its blob.
//! * **Size.** One screen in device pixels — `bb_save_words(W, H) * 4`
//!   ([`gsx_mcalc`]), not EmuTOS' `MENU_BUFFER_SIZE * gl_wchar * gl_hchar *
//!   gl_nplanes / 8` (`gemgsxif.c:98-104`). Both users clamp their rectangle
//!   to the screen, so `bb_set`'s "buffer too small, adjust height to fit:
//!   this will leave droppings" path (`:334-343`) is structurally
//!   unreachable; the guard in [`bb_save`] stays as a defence and counts
//!   ([`clipped_saves`]). Named deviation, `docs/COMPATIBILITY.md`.
//! * **Lifetime.** Never freed. EmuTOS frees it in `sh_toalpha`
//!   (`gemshlib.c:258`) and takes it again in the next `sh_tographic`;
//!   between two GEM programs it persists there too. Keeping it across an
//!   alpha interlude is the safe superset and unobservable (a program can
//!   only tell by using a freed block).
//! * **EL0.** `wind_get(WF_SCREEN)` publishes the block into the calling
//!   process's window lazily, read-write (`usermem::expose`, `PROT_G`,
//!   ADR-023) — see `window.rs`.
//! * **Without the buffer** (pool exhausted at boot, or a host test without
//!   the [`install_screen_buffer`] fixture) the alert and the drop-down fall
//!   back to a kernel-heap copy per save — the pre-A57 behaviour — and
//!   `WF_SCREEN` answers FALSE (`docs/COMPATIBILITY.md`, A54).

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use tos_types::{Long, MX_PREFTTRAM, MX_STRAM};

use crate::bdos::process::usermem::PAGE_SIZE;
use crate::vdi::backend::{ClipRect, RasterBackend};

/// Kernel high-half offset (mirror of `process::{arena,loader,usermem}` and
/// `aes::resource`): a BDOS-pool physical address `pa` is reachable from the
/// kernel at `pa + VA_OFFSET`.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

/// What `gsx_mret` reports: the buffer's kernel alias, its physical (pool)
/// address and its length in bytes (`gl_tmp.fd_addr` / `gl_mlen`).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ScreenBuffer {
    /// Kernel high-half VA of the (page-aligned) buffer.
    pub kva: u64,
    /// Physical address of the buffer — the pool address, what an EL1 caller
    /// of `wind_get(WF_SCREEN)` receives (ADR-021, raw PAs for kernel
    /// callers).
    pub pa: u64,
    /// Length in bytes (`gl_mlen`).
    pub len: usize,
}

// `gl_tmp.fd_addr` / `gl_mlen`. `BUF_LEN != 0` is "the buffer exists"; it is
// stored last (Release) and read first (Acquire), so a reader that sees the
// length sees the two addresses that go with it.
static BUF_KVA: AtomicU64 = AtomicU64::new(0);
static BUF_PA: AtomicU64 = AtomicU64::new(0);
static BUF_LEN: AtomicUsize = AtomicUsize::new(0);

/// How many times [`bb_save`] had to clip its height to the buffer —
/// EmuTOS' `KINFO("Menu/alert buffer too small …")` (`gemgsxif.c:338-342`)
/// as a counter, since `tos-api` has no serial log of its own. Zero by
/// construction at the screen-sized length.
static CLIPPED: AtomicU32 = AtomicU32::new(0);

#[inline]
fn page_round_up(v: u64) -> u64 {
    (v + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

/// `w`/`h` of the backend as the `i16` the raster ABI carries.
#[inline]
fn dim(v: u16) -> i16 {
    v.min(i16::MAX as u16) as i16
}

/// `gsx_mcalc` (`gemgsxif.c:98-104`), at NewTOS' size: the bytes one
/// `bb_save` of the **whole screen** needs — `stride16(W) * H * 2`, i.e.
/// [`crate::vdi::bb_save_words`]`(W, H) * 4` (640×480 → 614 400,
/// 1920×1080 → 4 147 200). Every rectangle either user saves is clamped to
/// the screen, so nothing can exceed it.
pub fn gsx_mcalc(backend: &dyn RasterBackend) -> usize {
    crate::vdi::bb_save_words(dim(backend.width()), dim(backend.height())) * 4
}

/// `gsx_malloc` (`gemgsxif.c:108-114`): take the buffer from the GEMDOS pool
/// (`dos_alloc_anyram` = `Mxalloc(len, MX_PREFTTRAM)`), once. Called from
/// `sh_tographic` (`gemshlib.c:236`); a second call returns the buffer
/// already taken (EmuTOS would take another, but only after `gsx_mfree`,
/// which NewTOS never does — module docs).
///
/// Must run **outside any process**: the block has to carry the kernel
/// owner cookie so no `Pterm` reclaims it (`bdos/mem/traps.rs`
/// `current_owner`). With a process running the call takes nothing and
/// answers `None` rather than mint a buffer that dies with that process.
///
/// `None` when there is no raster backend, no pool, or the pool is short
/// (`ENSMEM`); the AES then runs on the per-save fallback (module docs) and
/// `WF_SCREEN` answers FALSE. The kernel prints the outcome on its boot
/// log.
pub fn gsx_malloc() -> Option<ScreenBuffer> {
    if let Some(b) = gsx_mret() {
        return Some(b);
    }
    let backend = crate::host::host()?.raster()?;
    let len = gsx_mcalc(backend);
    if len == 0 {
        return None;
    }
    let running = crate::bdos::process::current();
    debug_assert!(running.is_null(), "gsx_malloc must run before the first process (kernel-owned block)");
    if !running.is_null() || !crate::bdos::mem::init::is_initialised() {
        return None;
    }
    // Page-round and over-allocate one page so the page-aligned base fits
    // inside the block (`rsrc_load`, `arena::alloc_tpa`): the pages the
    // buffer occupies then belong to nothing else, which an EL0 mapping
    // needs (ADR-023).
    let pages = page_round_up(len as u64);
    let want = pages + PAGE_SIZE;
    if want > Long::MAX as u64 {
        return None;
    }
    // SAFETY: boot CPU between the AUTO folder and the first GEM program —
    // no trap is in flight, the implicit BDOS lock is ours.
    let raw = unsafe { crate::bdos::mem::traps::mxalloc(want as Long, MX_PREFTTRAM) };
    if raw == 0 {
        return None;
    }
    let pa = page_round_up(raw as u64);
    let kva = pa.wrapping_add(VA_OFFSET);
    BUF_KVA.store(kva, Ordering::Relaxed);
    BUF_PA.store(pa, Ordering::Relaxed);
    BUF_LEN.store(len, Ordering::Release);
    Some(ScreenBuffer { kva, pa, len })
}

/// `gsx_mret` (`gemgsxif.c:125-129`): the buffer, or `None` while there is
/// none.
pub fn gsx_mret() -> Option<ScreenBuffer> {
    let len = BUF_LEN.load(Ordering::Acquire);
    if len == 0 {
        return None;
    }
    Some(ScreenBuffer {
        kva: BUF_KVA.load(Ordering::Relaxed),
        pa: BUF_PA.load(Ordering::Relaxed),
        len,
    })
}

/// Host-test fixture — the counterpart of `host::install_host`: hand the
/// AES a buffer that did not come from the pool (a host process has no BDOS
/// pool, and a host `Vec` does not lie under 2 GiB, so `gsx_malloc` cannot
/// run there). `pa` is what an EL1 `wind_get(WF_SCREEN)` will report.
///
/// # Safety
///
/// `ptr .. ptr + len` must stay valid and writable for the rest of the
/// process; `len` must be at least [`gsx_mcalc`] of the installed backend if
/// the [`bb_save`] guard is not the thing under test.
pub unsafe fn install_screen_buffer(ptr: *mut u8, pa: u64, len: usize) {
    BUF_KVA.store(ptr as u64, Ordering::Relaxed);
    BUF_PA.store(pa, Ordering::Relaxed);
    BUF_LEN.store(len, Ordering::Release);
}

/// How often [`bb_save`] clipped a rectangle's height to the buffer
/// (`gemgsxif.c:334-343`). Diagnostics only.
pub fn clipped_saves() -> u32 {
    CLIPPED.load(Ordering::Relaxed)
}

/// Where a [`Saved`] rectangle's pixels are.
enum Backing {
    /// The one screen buffer (`gl_tmp`): shared, not owned — a later
    /// [`bb_save`] overwrites it, as on EmuTOS.
    Screen,
    /// A transient pool block (`popup_blit`'s `dos_alloc_stram`), owned by
    /// the process that opened the pop-up / submenu and freed with the
    /// restore (`gemmnext.c:212`) — or with the drop, if the restore never
    /// comes.
    Pool { raw: Long, kva: u64 },
    /// No screen buffer exists: a kernel-heap copy per save, the pre-A57
    /// alert path, so the alert and the drop-down still put the screen back.
    Heap(alloc::vec::Vec<u32>),
}

/// A screen rectangle [`bb_save`] or [`popup_blit_save`] stored, to be put
/// back by [`Saved::restore`]. Carries the (possibly height-clipped)
/// rectangle the pixels belong to.
pub struct Saved {
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    backing: Backing,
}

impl Saved {
    /// The rectangle whose pixels are held: `(x, y, w, h)`.
    pub fn rect(&self) -> (i16, i16, i16, i16) {
        (self.x, self.y, self.w, self.h)
    }

    fn mem(&self) -> *const u32 {
        match &self.backing {
            Backing::Screen => BUF_KVA.load(Ordering::Relaxed) as *const u32,
            Backing::Pool { kva, .. } => *kva as *const u32,
            Backing::Heap(v) => v.as_ptr(),
        }
    }

    /// `bb_restore` (`gemgsxif.c:389-392`) / `popup_blit(tree, buf)`
    /// (`gemmnext.c:183-214`): copy the saved pixels back to their
    /// rectangle, cursor hidden for the copy (`bb_set`,
    /// `gemgsxif.c:367,378`). A pool block is released afterwards
    /// (`dos_free`, `:212`).
    pub fn restore(self, backend: &dyn RasterBackend) {
        let clip = ClipRect::full(backend.width(), backend.height());
        // SAFETY: `mem()` addresses `bb_save_words(w, h)` `u32` words the
        // matching save filled — the screen buffer (guard in `bb_save`), a
        // pool block sized for exactly this rectangle, or the heap `Vec`.
        unsafe {
            crate::vdi::bb_restore(backend, self.mem(), self.x, self.y, self.w, self.h, clip);
        }
        // `self` drops here: a pool block is freed by `Drop`.
    }
}

impl Drop for Saved {
    fn drop(&mut self) {
        if let Backing::Pool { raw, .. } = self.backing {
            // SAFETY: `raw` is the live pool block `popup_blit_save` took and
            // nothing else references it once the `Saved` goes.
            let _ = unsafe { crate::bdos::mem::traps::mfree(raw) };
        }
    }
}

/// `bb_save` (`gemgsxif.c:383-386` → `bb_set(TRUE, r)`): copy the screen
/// rectangle `(x, y, w, h)` — already clamped to the screen by the caller,
/// as `fm_alert`'s `rc_intersect(&gl_rscreen, &d)` (`gemfmalt.c:358`) and
/// `menu_sr`'s on-screen box do — into **the** screen buffer. `None` for an
/// empty rectangle.
///
/// If the rectangle does not fit the buffer, the height is clipped to what
/// fits, `sh = gl_mlen * sh / size` (`:336`), and the event counted
/// ([`clipped_saves`]); at the screen-sized length that cannot happen. With
/// no screen buffer at all the pixels go to a kernel-heap copy instead
/// (module docs).
pub fn bb_save(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16) -> Option<Saved> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let mut h = h;
    let backing = match gsx_mret() {
        Some(buf) => {
            let need = crate::vdi::bb_save_words(w, h) * 4;
            if need > buf.len {
                // `bb_set`: "adjust height to fit buffer: this will leave
                // droppings!" (`gemgsxif.c:334-337`).
                h = ((buf.len as u64 * h as u64) / need as u64) as i16;
                CLIPPED.fetch_add(1, Ordering::Relaxed);
                if h <= 0 {
                    return None;
                }
            }
            // SAFETY: `kva` addresses `len >= bb_save_words(w, h) * 4` bytes
            // of the buffer, which lives for the rest of the boot.
            unsafe { crate::vdi::bb_save(backend, buf.kva as *mut u32, x, y, w, h) };
            Backing::Screen
        }
        None => {
            let words = crate::vdi::bb_save_words(w, h);
            let mut v = alloc::vec![0u32; words];
            // SAFETY: `v` holds `bb_save_words(w, h)` `u32` words.
            unsafe { crate::vdi::bb_save(backend, v.as_mut_ptr(), x, y, w, h) };
            Backing::Heap(v)
        }
    };
    Some(Saved { x, y, w, h, backing })
}

/// `popup_blit(tree, NULL)` (`gemmnext.c:136-215`, the save direction):
/// copy the screen rectangle `(x, y, w, h)` — the caller's `popup_blit`
/// rectangle, clamped to the screen — into a **fresh ST-RAM pool block**
/// (`dos_alloc_stram(bufsize)`, `:155-163`; `bufsize = ((w + 15) / 16) *
/// 2 * h * gl_nplanes` = [`crate::vdi::bb_save_words`]`(w, h) * 4`). The
/// block belongs to the running process and goes with the restore.
///
/// `None` when the rectangle is empty or the pool cannot supply the block
/// (`buf == NULL`): the pop-up is then not shown at all (`mn_popup`,
/// `:329-334`, D-086 in `docs/api/aes/menu.md`), the submenu not displayed
/// (`display_submenu`, `:668-671`).
///
/// With **no pool at all** — a host test binary; on the board the BDOS pools
/// are up before the first AES call (`kernel/src/lib.rs` `bring_up_bdos_pools`
/// precedes the AUTO folder and the desktop) — the pixels go to a kernel-heap
/// copy, exactly as [`bb_save`] does without a screen buffer (`Backing::Heap`,
/// module docs). That keeps the D-086 "pool short → no pop-up/submenu"
/// contract intact where a pool exists, and lets the drop-down tracker's
/// submenu states (`mn_do` `SUBMENU_STATE`/`OUTSIDE_STATE`, A89) run in a
/// host test. A host `Vec` cannot stand in for a pool block: `mxalloc` hands
/// out `Long` pool keys the AES re-derives through the kernel high-half alias.
pub fn popup_blit_save(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16) -> Option<Saved> {
    if w <= 0 || h <= 0 {
        return None;
    }
    if !crate::bdos::mem::init::is_initialised() {
        let words = crate::vdi::bb_save_words(w, h);
        let mut v = alloc::vec![0u32; words];
        // SAFETY: `v` holds `bb_save_words(w, h)` `u32` words.
        unsafe { crate::vdi::bb_save(backend, v.as_mut_ptr(), x, y, w, h) };
        return Some(Saved { x, y, w, h, backing: Backing::Heap(v) });
    }
    let bytes = crate::vdi::bb_save_words(w, h) * 4;
    if bytes == 0 || bytes > Long::MAX as usize {
        return None;
    }
    // SAFETY: single-CPU AES path (trap context or the kernel desktop), the
    // implicit BDOS lock is held.
    let raw = unsafe { crate::bdos::mem::traps::mxalloc(bytes as Long, MX_STRAM) };
    if raw == 0 {
        return None;
    }
    let kva = (raw as u64).wrapping_add(VA_OFFSET);
    // SAFETY: `kva` aliases `bytes` live RW pool bytes of the block just
    // taken, exactly `bb_save_words(w, h)` `u32` words.
    unsafe { crate::vdi::bb_save(backend, kva as *mut u32, x, y, w, h) };
    Some(Saved { x, y, w, h, backing: Backing::Pool { raw, kva } })
}
