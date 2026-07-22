//! AES process descriptors (`AESPD`) — per-application AES state.
//!
//! Spec: [`docs/api/aes/application.md`]. Each GEM application that calls
//! `appl_init` gets a descriptor holding its id (= the GEMDOS process
//! id), its `p_flags` (`AP_OPEN`/`AP_MESAG`/…), its 8-char name (for
//! `appl_find`), and a per-application **FIFO message queue** (the standard
//! GEM message pipe used by `evnt_mesag`/`evnt_multi`/`appl_read`).
//!
//! The queue mirrors EmuTOS's `p_queue`/`p_qindex`/`p_qaddr` byte queue
//! (`aes/struct.h:150-152`, `QUEUE_SIZE = 128` bytes = 8 standard 16-byte
//! messages, `aes/struct.h:56`) and the enqueue/dequeue + WM_REDRAW
//! rectangle-coalescing rules of `doq`/`aqueue` (`aes/gemqueue.c:31-125`).
//! A single pending slot is just the 1-message case of this queue; when the
//! desktop (one AES app owning many windows) posts several `WM_REDRAW`s
//! before the event loop drains them, they no longer overwrite each other.
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

/// EmuTOS per-process queue size in bytes (`aes/struct.h:56`,
/// `p_queue[QUEUE_SIZE]` at `:152`).
pub const QUEUE_SIZE: usize = 128;
/// Queue capacity in whole 16-byte GEM messages: `QUEUE_SIZE / 16 = 8`
/// (all NewTOS AES messages are the standard 8-WORD length).
pub const QUEUE_MSGS: usize = QUEUE_SIZE / (MSG_WORDS * 2);

// GEM message types that `doq` coalesces when enqueued (`include/aesdefs.h`,
// used by the queue-union rule in `aes/gemqueue.c:45-67`).
const WM_REDRAW: i16 = 20;
const WM_ARROWED: i16 = 24;
const WM_HSLID: i16 = 25;
const WM_VSLID: i16 = 26;

/// Per-application GEM message FIFO — the reimplementation of EmuTOS's
/// `p_queue`/`p_qindex` byte queue. Holds up to [`QUEUE_MSGS`] standard
/// 16-byte (8-WORD) messages; `count` is the number currently queued (the
/// analog of `p_qindex`, in whole messages instead of bytes since every
/// NewTOS message is exactly 16 bytes).
#[derive(Clone, Copy)]
pub struct MsgQueue {
    msgs: [[i16; MSG_WORDS]; QUEUE_MSGS],
    count: usize,
}

impl MsgQueue {
    const EMPTY: MsgQueue = MsgQueue {
        msgs: [[0; MSG_WORDS]; QUEUE_MSGS],
        count: 0,
    };

    /// Number of messages currently queued (the analog of `p_qindex`; a read
    /// is ready when this is `> 0`, EmuTOS `aes/gemqueue.c:99`).
    pub fn pending(&self) -> usize {
        self.count
    }

    /// Peek the front (oldest) message without removing it. Used by the P5
    /// test hook; the real `evnt_*`/`appl_read` paths [`pop`](Self::pop).
    pub fn front(&self) -> Option<[i16; MSG_WORDS]> {
        (self.count > 0).then_some(self.msgs[0])
    }

    /// Enqueue a 16-byte message — the enqueue half of EmuTOS `doq`
    /// (`aes/gemqueue.c:37-73`). Appends `msg` at the tail, except that for a
    /// `WM_REDRAW`/`WM_ARROWED`/`WM_HSLID`/`WM_VSLID` (`:45`) it first scans
    /// the messages already queued and coalesces:
    ///
    /// * a `WM_REDRAW` for the **same window handle** (`msg[3]`): union the
    ///   new rectangle (`msg[4..8]`) into that existing entry via `rc_union`
    ///   (`aes/gemqueue.c:52-56`, `util/rectfunc.c:84-97`) and drop the new
    ///   message — this is the fix for redraw flooding / lost content redraws;
    /// * otherwise an existing `WM_ARROWED`: overwrite it with the new message
    ///   (`aes/gemqueue.c:63-67`).
    ///
    /// Returns `false` when the queue is full. EmuTOS instead **blocks** the
    /// sender until a dequeue frees room (`aqueue`, `aes/gemqueue.c:120-124`,
    /// via `p_qnq`/`evinsert`); NewTOS's bring-up has no AES scheduler to park
    /// a synchronous kernel-side poster, so it reports the failure rather than
    /// overwrite or silently drop an in-queue message. With same-window redraw
    /// coalescing and a per-loop drain, a full queue is unreachable in the
    /// desktop's actual usage (at most `MAX_WINDOWS` distinct-window redraws
    /// per broadcast).
    pub fn push(&mut self, msg: [i16; MSG_WORDS]) -> bool {
        let t = msg[0];
        if t == WM_REDRAW || t == WM_ARROWED || t == WM_HSLID || t == WM_VSLID {
            for i in 0..self.count {
                let om = &mut self.msgs[i];
                if om[0] == WM_REDRAW && msg[3] == om[3] {
                    // Union the new rectangle into the existing entry.
                    rc_union_into(&msg, om);
                    return true;
                } else if om[0] == WM_ARROWED {
                    // Replace the pending arrow with the newer message.
                    *om = msg;
                    return true;
                }
            }
        }
        if self.count >= QUEUE_MSGS {
            return false;
        }
        self.msgs[self.count] = msg;
        self.count += 1;
        true
    }

    /// Dequeue the front (oldest) message — the dequeue half of EmuTOS `doq`
    /// (`aes/gemqueue.c:76-81`): return `p_queue[0]` and shift the residual
    /// messages down. `None` when the queue is empty.
    pub fn pop(&mut self) -> Option<[i16; MSG_WORDS]> {
        if self.count == 0 {
            return None;
        }
        let front = self.msgs[0];
        for i in 1..self.count {
            self.msgs[i - 1] = self.msgs[i];
        }
        self.count -= 1;
        Some(front)
    }

    /// Discard every queued message (used on `appl_init` and by the test hook).
    pub fn clear(&mut self) {
        self.count = 0;
    }
}

/// Union the rectangle in `p1[4..8]` into `p2[4..8]`, storing the result in
/// `p2` — EmuTOS `rc_union(p1, p2)` (`util/rectfunc.c:84-97`), used by `doq`
/// to coalesce two same-window `WM_REDRAW` rectangles (`aes/gemqueue.c:54`).
fn rc_union_into(p1: &[i16; MSG_WORDS], p2: &mut [i16; MSG_WORDS]) {
    let tw = (p1[4] + p1[6]).max(p2[4] + p2[6]);
    let th = (p1[5] + p1[7]).max(p2[5] + p2[7]);
    let tx = p1[4].min(p2[4]);
    let ty = p1[5].min(p2[5]);
    p2[4] = tx;
    p2[5] = ty;
    p2[6] = tw - tx;
    p2[7] = th - ty;
}

/// One AES application descriptor.
#[derive(Clone, Copy)]
pub struct Aespd {
    pub in_use: bool,
    pub pid: i16,
    pub flags: u16,
    pub name: [u8; 8],
    /// The application's pending GEM message FIFO (`p_queue`/`p_qindex`).
    pub queue: MsgQueue,
}

impl Aespd {
    const EMPTY: Aespd = Aespd {
        in_use: false,
        pid: 0,
        flags: 0,
        name: [b' '; 8],
        queue: MsgQueue::EMPTY,
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

/// Address of the AES process-descriptor table — NewTOS's nearest equivalent
/// of EmuTOS's AES global data area `THEGLO D` (`aes/gemlib.h:116-144`, whose
/// `g_int[]` holds the `AESPD` process descriptors; a single unified `D` struct
/// with no NewTOS counterpart — our AES state is split across statics, of which
/// this descriptor table is the process-management core). `appl_init` reports it
/// in `ap_3resv` (EmuTOS `pglobal->ap_3resv = (LONG)&D`, `gemsuper.c:98`).
pub fn table_addr() -> u64 {
    TABLE.0.get() as u64
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
    slot.queue.clear();
    Some(slot)
}

/// Post a 16-byte GEM message into `pid`'s queue — the routing point every
/// AES message post goes through (the `ap_rdwr(MU_SDMSG)` → `aqueue` → `doq`
/// enqueue path, `aes/gemqueue.c:85-125,31-74`). Returns `false` if `pid`
/// has no descriptor or the queue is full (see [`MsgQueue::push`]).
///
/// # Safety
/// Single trap path.
pub unsafe fn post_msg(pid: i16, msg: [i16; MSG_WORDS]) -> bool {
    // SAFETY: caller upholds the single-trap-path invariant.
    match unsafe { by_pid(pid) } {
        Some(p) => p.queue.push(msg),
        None => false,
    }
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
