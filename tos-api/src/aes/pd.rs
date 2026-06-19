//! AES process descriptors (`AESPD`) — per-application AES state.
//!
//! Spec: [`docs/api/aes/application.md`]. Each GEM application that calls
//! `appl_init` gets a descriptor holding its id (= the GEMDOS process
//! id), its `p_flags` (`AP_OPEN`/`AP_MESAG`/…), its 8-char name (for
//! `appl_find`), and a single pending 16-byte message slot (the standard
//! GEM message pipe used by `evnt_mesag`).
//!
//! Backed by a static slot table accessed only from the single-CPU,
//! IRQ-off GEM-trap path (the same discipline as the VDI/`Vwk` table).

use core::cell::UnsafeCell;

/// `p_flags` bit: the application is between `appl_init` and `appl_exit`.
pub const AP_OPEN: u16 = 0x0001;
/// `p_flags` bit: the application has waited for a message.
pub const AP_MESAG: u16 = 0x0002;

/// Maximum concurrently-registered AES applications.
pub const MAX_APPS: usize = 8;

/// Length of the standard GEM message (8 WORDs).
pub const MSG_WORDS: usize = 8;

/// One AES application descriptor.
#[derive(Clone, Copy)]
pub struct Aespd {
    pub in_use: bool,
    pub pid: i16,
    pub flags: u16,
    pub name: [u8; 8],
    /// A single pending 16-byte message (`evnt_mesag` fast path).
    pub msg: [i16; MSG_WORDS],
    pub msg_pending: bool,
}

impl Aespd {
    const EMPTY: Aespd = Aespd {
        in_use: false,
        pid: 0,
        flags: 0,
        name: [b' '; 8],
        msg: [0; MSG_WORDS],
        msg_pending: false,
    };
}

struct Table(UnsafeCell<[Aespd; MAX_APPS]>);
// SAFETY: accessed only from the single-CPU, IRQ-off GEM-trap path.
unsafe impl Sync for Table {}
static TABLE: Table = Table(UnsafeCell::new([Aespd::EMPTY; MAX_APPS]));

/// Borrow the descriptor table.
///
/// # Safety
/// Single trap path (no re-entrancy).
#[allow(clippy::mut_from_ref)]
unsafe fn table() -> &'static mut [Aespd; MAX_APPS] {
    // SAFETY: see `Table`'s Sync impl.
    unsafe { &mut *TABLE.0.get() }
}

/// Register an application by `pid`, returning a mutable reference to its
/// (possibly reused) descriptor, or `None` if the table is full. Sets
/// `in_use`, `pid` and `AP_OPEN`; clears the rest.
///
/// # Safety
/// Single trap path.
pub unsafe fn open(pid: i16) -> Option<&'static mut Aespd> {
    // SAFETY: caller upholds the single-trap-path invariant.
    let t = unsafe { table() };
    // Reuse an existing descriptor for this pid, else claim a free slot.
    let idx = t
        .iter()
        .position(|s| s.in_use && s.pid == pid)
        .or_else(|| t.iter().position(|s| !s.in_use))?;
    let slot = &mut t[idx];
    if !(slot.in_use && slot.pid == pid) {
        *slot = Aespd::EMPTY;
        slot.in_use = true;
        slot.pid = pid;
    }
    slot.flags = AP_OPEN;
    slot.msg_pending = false;
    Some(slot)
}

/// Look up an application descriptor by pid.
///
/// # Safety
/// Single trap path.
pub unsafe fn by_pid(pid: i16) -> Option<&'static mut Aespd> {
    // SAFETY: single trap path.
    let t = unsafe { table() };
    t.iter_mut().find(|s| s.in_use && s.pid == pid)
}

/// Find an application by its 8-char name, returning its pid or `-1`.
///
/// # Safety
/// Single trap path.
pub unsafe fn find_by_name(name: &[u8; 8]) -> i16 {
    // SAFETY: single trap path.
    let t = unsafe { table() };
    t.iter()
        .find(|s| s.in_use && &s.name == name)
        .map(|s| s.pid)
        .unwrap_or(-1)
}

/// Deregister an application by pid (clears its descriptor).
///
/// # Safety
/// Single trap path.
pub unsafe fn close(pid: i16) {
    // SAFETY: single trap path.
    let t = unsafe { table() };
    if let Some(slot) = t.iter_mut().find(|s| s.in_use && s.pid == pid) {
        *slot = Aespd::EMPTY;
    }
}
