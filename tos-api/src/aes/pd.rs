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

/// Length of the standard GEM message (8 WORDs = 16 bytes) — the normal case
/// for every internal AES poster (`WM_REDRAW`, menu, keyboard, …).
pub const MSG_WORDS: usize = 8;

/// EmuTOS per-process queue size in bytes (`aes/struct.h:56`,
/// `p_queue[QUEUE_SIZE]` at `:152`). Also the largest a single message may be:
/// EmuTOS `doq` memcpys up to `qpb_cnt` bytes into a queue of this size
/// (`aes/gemqueue.c:36,97`).
pub const QUEUE_SIZE: usize = 128;
/// Largest single message in WORDs: `QUEUE_SIZE / 2 = 64`. `appl_write` clamps
/// `AP_LENGTH` to this — the per-message FIFO limit (`aes/gemqueue.c` never
/// enqueues more than `QUEUE_SIZE` bytes). The standard 16-byte message uses
/// only the first [`MSG_WORDS`] of it.
pub const MAX_MSG_WORDS: usize = QUEUE_SIZE / 2;
/// Maximum number of messages the FIFO holds at once, kept at the original
/// `QUEUE_SIZE / 16 = 8` (enough for one `WM_REDRAW` per open window before the
/// event loop drains them). Unlike EmuTOS's single 128-byte byte-queue, each of
/// these slots can independently hold a full [`MAX_MSG_WORDS`]-WORD message, so
/// the FIFO over-provisions (8 × `QUEUE_SIZE` B) rather than sharing one
/// 128-byte budget — a memory-for-simplicity trade of the slot model.
pub const QUEUE_MSGS: usize = QUEUE_SIZE / (MSG_WORDS * 2);

// GEM message types that `doq` coalesces when enqueued (`include/aesdefs.h`,
// used by the queue-union rule in `aes/gemqueue.c:45-67`).
const WM_REDRAW: i16 = 20;
const WM_ARROWED: i16 = 24;
const WM_HSLID: i16 = 25;
const WM_VSLID: i16 = 26;

/// Per-application GEM message FIFO — the reimplementation of EmuTOS's
/// `p_queue`/`p_qindex` byte queue. Holds up to [`QUEUE_MSGS`] messages, each
/// of up to [`MAX_MSG_WORDS`] WORDs (`QUEUE_SIZE` bytes); the standard case is
/// the 16-byte (8-WORD) GEM message. `count` is the number of messages
/// currently queued (the analog of `p_qindex`, kept in whole messages here
/// because the slot model delivers a message atomically rather than as a byte
/// stream). Unlike EmuTOS the FIFO cannot split a message across reads: a
/// message is enqueued and dequeued whole (see [`Self::pop_into`]).
#[derive(Clone, Copy)]
pub struct MsgQueue {
    /// Each queued message, held in up to [`MAX_MSG_WORDS`] WORDs
    /// (`QUEUE_SIZE` bytes); only the first `lens[i]` WORDs are valid.
    msgs: [[i16; MAX_MSG_WORDS]; QUEUE_MSGS],
    /// Valid WORD length of each queued message (`msgs[i][..lens[i]]`). The
    /// standard GEM message is [`MSG_WORDS`]; `appl_write` may queue up to
    /// [`MAX_MSG_WORDS`].
    lens: [usize; QUEUE_MSGS],
    count: usize,
}

impl MsgQueue {
    const EMPTY: MsgQueue = MsgQueue {
        msgs: [[0; MAX_MSG_WORDS]; QUEUE_MSGS],
        lens: [0; QUEUE_MSGS],
        count: 0,
    };

    /// Number of messages currently queued (the analog of `p_qindex`; a read
    /// is ready when this is `> 0`, EmuTOS `aes/gemqueue.c:99`).
    pub fn pending(&self) -> usize {
        self.count
    }

    /// Peek the standard 16-byte front (oldest) message without removing it.
    /// Used by the P5 test hook; the real `evnt_*`/`appl_read` paths
    /// [`pop`](Self::pop)/[`pop_into`](Self::pop_into).
    pub fn front(&self) -> Option<[i16; MSG_WORDS]> {
        if self.count == 0 {
            return None;
        }
        let mut out = [0i16; MSG_WORDS];
        let n = self.lens[0].min(MSG_WORDS);
        out[..n].copy_from_slice(&self.msgs[0][..n]);
        Some(out)
    }

    /// Enqueue the standard 16-byte GEM message `msg` — [`Self::enqueue`] at
    /// the standard [`MSG_WORDS`] length. Used by every internal poster
    /// (`WM_REDRAW`, menu, keyboard). Returns `false` when the queue is full.
    pub fn push(&mut self, msg: [i16; MSG_WORDS]) -> bool {
        self.enqueue(&msg, MSG_WORDS)
    }

    /// Enqueue a variable-length message: the first `words` WORDs of `src` —
    /// the `appl_write`/`ap_rdwr(MU_SDMSG)` → `doq` path (EmuTOS
    /// `aes/gemqueue.c:31-73`, which memcpys `qpb_cnt` bytes). `words` is
    /// clamped to [`MAX_MSG_WORDS`] (the per-message FIFO limit) and to
    /// `src.len()`. Returns `false` when the queue is full.
    pub fn push_words(&mut self, src: &[i16], words: usize) -> bool {
        self.enqueue(src, words.min(src.len()))
    }

    /// Core enqueue for [`Self::push`] and [`Self::push_words`] — the enqueue
    /// half of EmuTOS `doq` (`aes/gemqueue.c:37-73`). Appends the first `words`
    /// WORDs of `src` at the tail, except that a standard-length
    /// `WM_REDRAW`/`WM_ARROWED`/`WM_HSLID`/`WM_VSLID` (`:45`) is first coalesced
    /// into an already-queued standard message:
    ///
    /// * a `WM_REDRAW` for the **same window handle** (`src[3]`): union the new
    ///   rectangle (`src[4..8]`) into that entry via `rc_union`
    ///   (`aes/gemqueue.c:52-56`, `util/rectfunc.c:84-97`) and drop the new
    ///   message — the fix for redraw flooding / lost content redraws;
    /// * otherwise an existing `WM_ARROWED`: overwrite it (`aes/gemqueue.c:63-67`).
    ///
    /// Only standard ([`MSG_WORDS`]-long) messages take part in coalescing — the
    /// coalesced message types are always 16 bytes.
    ///
    /// Returns `false` when the queue is full. EmuTOS instead **blocks** the
    /// sender until a dequeue frees room (`aqueue`, `aes/gemqueue.c:120-124`,
    /// via `p_qnq`/`evinsert`); NewTOS's bring-up has no AES scheduler to park
    /// a synchronous kernel-side poster, so it reports the failure rather than
    /// overwrite or silently drop an in-queue message. With same-window redraw
    /// coalescing and a per-loop drain, a full queue is unreachable in the
    /// desktop's actual usage (at most `MAX_WINDOWS` distinct-window redraws
    /// per broadcast).
    fn enqueue(&mut self, src: &[i16], words: usize) -> bool {
        let words = words.min(MAX_MSG_WORDS);
        if words == MSG_WORDS {
            let t = src[0];
            if t == WM_REDRAW || t == WM_ARROWED || t == WM_HSLID || t == WM_VSLID {
                for i in 0..self.count {
                    if self.lens[i] != MSG_WORDS {
                        continue; // only standard messages coalesce
                    }
                    let om = &mut self.msgs[i];
                    if om[0] == WM_REDRAW && src[3] == om[3] {
                        // Union the new rectangle into the existing entry.
                        rc_union_into(src, om);
                        return true;
                    } else if om[0] == WM_ARROWED {
                        // Replace the pending arrow with the newer message.
                        om[..MSG_WORDS].copy_from_slice(&src[..MSG_WORDS]);
                        return true;
                    }
                }
            }
        }
        if self.count >= QUEUE_MSGS {
            return false;
        }
        let slot = &mut self.msgs[self.count];
        slot[..words].copy_from_slice(&src[..words]);
        for w in &mut slot[words..] {
            *w = 0;
        }
        self.lens[self.count] = words;
        self.count += 1;
        true
    }

    /// Dequeue the standard 16-byte front (oldest) message — the dequeue half
    /// of EmuTOS `doq` (`aes/gemqueue.c:76-81`). Returns the first [`MSG_WORDS`]
    /// WORDs of `p_queue[0]` and shifts the residual messages down; `None` when
    /// the queue is empty. Used by the internal `evnt_mesag`/`evnt_multi` paths,
    /// which read exactly 16 bytes (EmuTOS `ev_mesag` → `ap_rdwr(…,16,…)`).
    pub fn pop(&mut self) -> Option<[i16; MSG_WORDS]> {
        if self.count == 0 {
            return None;
        }
        let mut out = [0i16; MSG_WORDS];
        let n = self.lens[0].min(MSG_WORDS);
        out[..n].copy_from_slice(&self.msgs[0][..n]);
        self.remove_front();
        Some(out)
    }

    /// Dequeue the front message into `dst`, copying at most `dst.len()` WORDs
    /// (the `appl_read`/`ap_rdwr(MU_MESAG,…,length,…)` caller's requested
    /// length); returns the number of WORDs delivered, or `None` when the queue
    /// is empty. The whole message is removed even if `dst` is shorter than it —
    /// the slot FIFO delivers a message atomically, without EmuTOS's byte-stream
    /// partial reads (see the type-level note).
    pub fn pop_into(&mut self, dst: &mut [i16]) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let n = self.lens[0].min(dst.len());
        dst[..n].copy_from_slice(&self.msgs[0][..n]);
        self.remove_front();
        Some(n)
    }

    /// Remove the front message, shifting the residual messages down (EmuTOS
    /// `doq` dequeue, `aes/gemqueue.c:76-81`).
    fn remove_front(&mut self) {
        for i in 1..self.count {
            self.msgs[i - 1] = self.msgs[i];
            self.lens[i - 1] = self.lens[i];
        }
        self.count -= 1;
    }

    /// Discard every queued message (explicit queue reset and test hook).
    pub fn clear(&mut self) {
        self.count = 0;
    }
}

/// Union the rectangle in `p1[4..8]` into `p2[4..8]`, storing the result in
/// `p2` — EmuTOS `rc_union(p1, p2)` (`util/rectfunc.c:84-97`), used by `doq`
/// to coalesce two same-window `WM_REDRAW` rectangles (`aes/gemqueue.c:54`).
/// Both slices are the coalesced standard messages, so index `4..8` is valid.
fn rc_union_into(p1: &[i16], p2: &mut [i16]) {
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
/// (possibly reused) descriptor, or `None` if the table is full. Initialises
/// a new slot; on an existing one only sets `AP_OPEN`, preserving its queue,
/// name and other flags (EmuTOS `ap_init`, `aes/gemaplib.c:54-63`).
///
/// The application's loaded resources are not part of this descriptor
/// (EmuTOS's `AESPD` has no resource field either, `aes/struct.h:119-155`);
/// they are keyed by the same pid in `super::resource` and dropped at the
/// process's end (`resource::on_process_exit`), so a reused descriptor never
/// carries a previous tenant's resource (A39 R6).
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
    slot.flags |= AP_OPEN;
    Some(slot)
}

#[cfg(test)]
mod init_tests {
    use super::*;

    #[test]
    fn reopening_preserves_flags_and_queue_even_when_the_table_is_full() {
        // This unit-test binary has no other descriptor-table users. Negative
        // test ids also keep the fixtures distinct from real process slots.
        unsafe {
            for n in 0..MAX_APPS {
                let app = open(-100 - n as i16).expect("fresh slot");
                assert_eq!(app.flags, AP_OPEN);
            }
            let message = [0x4711, 1, 0, 2, 3, 4, 5, 6];
            {
                let app = by_pid(-100).unwrap();
                app.flags |= AP_MESAG;
                app.name = *b"INITTEST";
                assert!(app.queue.push(message));
            }
            assert!(open(-200).is_none(), "a full table rejects a new id");
            {
                let app = open(-100).expect("an existing id needs no free slot");
                assert_eq!(app.flags, AP_OPEN | AP_MESAG);
                assert_eq!(app.name, *b"INITTEST");
                let mut received = [0; MSG_WORDS];
                assert_eq!(app.queue.pop_into(&mut received), Some(MSG_WORDS));
                assert_eq!(received, message);
            }
            for n in 0..MAX_APPS {
                close(-100 - n as i16);
            }
        }
    }
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
    let posted = match unsafe { by_pid(pid) } {
        Some(p) => p.queue.push(msg),
        None => false,
    };
    // AES trap tracer: record the enqueue (type, sender, target, and whether
    // the FIFO took it). No-op without the `aes-trace` feature.
    super::trace::msg_post(pid, &msg, MSG_WORDS, posted);
    if posted {
        wake_mesg_waiter(pid);
    }
    posted
}

/// Post a variable-length message into `pid`'s queue — the `appl_write` /
/// `ap_rdwr(MU_SDMSG)` path with a caller-supplied byte length (EmuTOS
/// `doq` memcpys `qpb_cnt` bytes, `aes/gemqueue.c:31-73`). Enqueues the first
/// `words` WORDs of `src` with the standard redraw/arrow coalescing
/// ([`MsgQueue::push_words`]) and, on success, wakes an AES client blocked on
/// `pid`'s FIFO through the **same** centralized wake as [`post_msg`] — so
/// `appl_write` no longer bypasses it (design §5.2). Returns `false` if `pid`
/// has no descriptor or the queue is full.
///
/// # Safety
/// Single trap path.
pub unsafe fn post_words(pid: i16, src: &[i16], words: usize) -> bool {
    // SAFETY: caller upholds the single-trap-path invariant.
    let posted = match unsafe { by_pid(pid) } {
        Some(p) => p.queue.push_words(src, words),
        None => false,
    };
    // AES trap tracer, as in `post_msg` (this is the `appl_write` half).
    super::trace::msg_post(pid, src, words, posted);
    if posted {
        wake_mesg_waiter(pid);
    }
    posted
}

/// Wake any AES client blocked in `evnt_mesag`/`evnt_multi` on `pid`'s message
/// FIFO (WAITING → READY, sleep/wake substrate §5.2). The single wake point for
/// every message poster ([`post_msg`]/[`post_words`]), matching EmuTOS's
/// `aqueue`→`azombie`→`signal` (`emutos-cooperative-model.md` §3.3/§4.3). Not
/// load-bearing in step 3a (single context: the waiter's own idle re-check
/// drains the queue), but wired now so step 4's real peer switch needs no
/// `event.rs`/`appl_write` change.
fn wake_mesg_waiter(pid: i16) {
    crate::bdos::process::sched::wake(crate::bdos::process::sched::wait_mesg(pid));
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

/// Deregister an application by pid — EmuTOS `ap_exit`
/// (`aes/gemaplib.c:251-261`):
///
/// ```c
/// wm_update(BEG_UPDATE);
/// mn_cleanup();
/// wait_for_accs(AP_ACCLOSE);      /* block until all DAs have seen AC_CLOSE */
/// if (rlr->p_qindex)
///     ap_rdwr(MU_MESAG, rlr, rlr->p_qindex, (WORD *)D.g_valstr);
/// wm_update(END_UPDATE);
/// all_run();
/// rlr->p_flags &= ~AP_OPEN;       /* say appl_exit() is done */
/// ```
///
/// Two things happen to the descriptor and no more: **the queue is drained**
/// (the `ap_rdwr(MU_MESAG, …)` reads every pending message into the scratch
/// buffer `D.g_valstr`, i.e. throws it away) and **`AP_OPEN` is cleared**.
/// `p_pid`, `p_name` and the remaining flags survive, and the `AESPD` itself
/// is never freed — EmuTOS's descriptors are fixed entries of
/// `D.g_int[]`/`D.g_acc[]` addressed by index (`pd_index`, `aes/gempd.c:32-35`)
/// and `fpdnm` walks that array with no liveness test (`:38-57`), so a message
/// addressed to a pid always finds its descriptor, before and after
/// `appl_exit`.
///
/// This matters here and not only in theory: NewTOS's desktop runs in the
/// kernel boot context and gets pid 1 (`appl::current_pid`), the first program
/// it launches gets GEMDOS slot 0 and therefore pid 1 as well
/// ([`super::pid_of_slot`]) — the *same* descriptor, exactly as EmuTOS's
/// `sh_ldapp` renames the one `rlr` for each successive program
/// (`aes/gemshlib.c:633-636`). Destroying the descriptor here used to take the
/// desktop's message pipe with it when a program quit (A29): `pd::by_pid(1)`
/// went `None`, and from then on every `WM_REDRAW`/`WM_CLOSED`/`WM_MOVED`/
/// `MN_SELECTED` the AES posted to the desktop was silently dropped.
///
/// `mn_cleanup` (`gemmnlib.c:740-758`) and `wait_for_accs` concern desk
/// accessories, which NewTOS has none of; `wm_update`/`all_run` bracket the
/// drain against the AES's own multitasking, which a synchronous drain on the
/// single trap path cannot race.
///
/// # Safety
/// Single trap path.
pub unsafe fn close(pid: i16) {
    // SAFETY: single trap path.
    let t = unsafe { table() };
    if let Some(slot) = t.iter_mut().find(|s| s.in_use && s.pid == pid) {
        slot.queue.clear(); // ap_rdwr(MU_MESAG, rlr, rlr->p_qindex, …)
        slot.flags &= !AP_OPEN; // rlr->p_flags &= ~AP_OPEN
    }
}

/// `sh_ldapp`'s `AP_OPEN` cleanup (`aes/gemshlib.c:650-657`) — the program
/// that has just terminated called `appl_init()` but never `appl_exit()`:
///
/// ```c
/// if (rlr->p_flags & AP_OPEN) {
///     mn_cleanup();
///     if (rlr->p_qindex) ap_rdwr(MU_MESAG, rlr, rlr->p_qindex, (WORD *)D.g_valstr);
///     rlr->p_flags &= ~AP_OPEN;
/// }
/// ```
///
/// EmuTOS does "the important parts of `appl_exit`" on the program's behalf,
/// because `rlr` (its single AESPD 0) is about to be reused by the next
/// program and a stale `AP_OPEN` plus an undrained queue would corrupt that
/// program's state. Those important parts are the same two the full `ap_exit`
/// performs — drain the queue, clear `AP_OPEN` — so this is [`close`]'s body
/// under EmuTOS's `AP_OPEN` guard. `mn_cleanup` (`gemmnlib.c:740-758`) sends
/// `AC_CLOSE` to every registered desk accessory and frees the submenu buffer;
/// NewTOS has neither.
///
/// Like `ap_exit`, this **keeps the descriptor**: `p_pid` and `p_name` stay, so
/// the next program to occupy the slot finds the pd to rename
/// (`aes/gemshlib.c:633`) and — the case that made this a P1 — the desktop,
/// which shares pid 1 with the first program it launches, still has a message
/// pipe after that program quits. See [`close`] for the full reasoning.
///
/// A descriptor without `AP_OPEN` (the program did call `appl_exit`, or never
/// called `appl_init`) is left alone, as EmuTOS's `if` leaves it alone.
///
/// # Safety
/// Single trap path.
pub unsafe fn close_if_open(pid: i16) {
    // SAFETY: single trap path.
    let t = unsafe { table() };
    if let Some(slot) = t
        .iter_mut()
        .find(|s| s.in_use && s.pid == pid && (s.flags & AP_OPEN) != 0)
    {
        slot.queue.clear(); // ap_rdwr(MU_MESAG, rlr, rlr->p_qindex, …)
        slot.flags &= !AP_OPEN; // rlr->p_flags &= ~AP_OPEN
    }
}

/// The 8-char `p_name` length (EmuTOS `aes/struct.h:111`, `AP_NAMELEN`).
const AP_NAMELEN: usize = 8;

/// The filename portion of a launch path — EmuTOS `sh_name` (`aes/gemshlib.c:594`):
/// everything after the last path separator (`\`). A bare filename (no
/// separator) is returned whole.
fn base_name(path: &[u8]) -> &[u8] {
    let start = path
        .iter()
        .rposition(|&c| c == b'\\')
        .map(|i| i + 1)
        .unwrap_or(0);
    &path[start..]
}

/// Fill the 8-char `p_name` `dst` from the program name `src` — EmuTOS
/// `p_nameit` (`aes/gempd.c:77-86`): copy up to [`AP_NAMELEN`] chars, stopping
/// at the first `.` (drop the extension), then space-pad the remainder.
fn p_nameit(dst: &mut [u8; AP_NAMELEN], src: &[u8]) {
    let mut i = 0;
    while i < AP_NAMELEN {
        match src.get(i) {
            Some(&c) if c != 0 && c != b'.' => {
                dst[i] = c;
                i += 1;
            }
            _ => break,
        }
    }
    while i < AP_NAMELEN {
        dst[i] = b' ';
        i += 1;
    }
}

/// Hand the AES process descriptor for `pid` over to the program about to run
/// from `path` — EmuTOS `sh_ldapp`'s three lines between finding the program
/// and `dos_exec` (`aes/gemshlib.c:633-636`):
///
/// ```c
/// p_nameit(rlr, fname);
/// p_setappdir(rlr, D.s_cmd);
/// rlr->p_flags = 0;
/// ```
///
/// Two of the three land here. `p_name` becomes the 8-char, space-padded,
/// extensionless program name so a later `appl_find(name)` finds the process
/// ([`find_by_name`] / EmuTOS `fpdnm`, `aes/gempd.c:38-57`), and **`p_flags` is
/// cleared**: the new occupant has not called `appl_init` yet, so none of
/// `AP_OPEN`/`AP_MESAG`/`AP_ACCLOSE` may still be set from the previous one.
/// (`p_setappdir` is the launcher's own step,
/// [`super::resource::set_app_dir`].)
///
/// The flag reset is load-bearing because NewTOS crosses this boundary on the
/// **same** descriptor EmuTOS does: the desktop runs in the kernel boot context
/// with pid 1 (`appl::current_pid`) and the first program it launches gets
/// GEMDOS slot 0, hence pid 1 too ([`super::pid_of_slot`]) — exactly as EmuTOS
/// renames the one `rlr`. Its one consumer is `sh_ldapp`'s post-`dos_exec`
/// cleanup ([`close_if_open`], `aes/gemshlib.c:650-657`), whose whole question
/// is "did **this** program call `appl_init` without `appl_exit`?". Without the
/// reset it read the *previous* occupant's `AP_OPEN` and drained a queue that
/// had nothing to do with the program that just ended (A28).
///
/// `p_flags = 0` does **not** touch the queue — draining it is the other half
/// of EmuTOS's boundary, the desktop's own `appl_exit()` in its epilog
/// (`desk/deskmain.c:2166` → `ap_exit`, [`close`]).
///
/// If no descriptor exists for `pid` yet, a free slot is claimed and
/// initialised (`in_use`, `pid`, name; `flags = 0`, empty queue), mirroring
/// `getpd` linking a fresh pd for a new process (`aes/gempd.c:59-68`). A full
/// table simply leaves the name unregistered.
///
/// # Safety
/// Single trap / launcher path (no re-entrancy), same discipline as [`open`].
pub unsafe fn register(pid: i16, path: &[u8]) {
    // SAFETY: caller upholds the single-path invariant.
    let t = unsafe { table() };
    let idx = match t.iter().position(|s| s.in_use && s.pid == pid) {
        Some(i) => i,
        None => match t.iter().position(|s| !s.in_use) {
            Some(i) => {
                t[i] = Aespd::EMPTY;
                t[i].in_use = true;
                t[i].pid = pid;
                i
            }
            None => return, // table full: the name is simply not registered
        },
    };
    p_nameit(&mut t[idx].name, base_name(path)); // p_nameit(rlr, fname)
    t[idx].flags = 0; // rlr->p_flags = 0
}
