# AES shared state — process descriptors, ids, the message pipe

> **PHASE-00b D8 (AES).** Source: EmuTOS @ `1a7484ab` —
> `aes/aesvars.h` (private AES globals), `aes/geminit.c` (start-up init),
> `aes/struct.h` (`AESPD`/`QPB`/`EVB`), `aes/gemlib.h` (`THEGLO`/`AESPROCESS`),
> `aes/gempd.c` (descriptor lookup), `aes/gemqueue.c` (the message pipe).
>
> Referenced by every AES manager spec for the shared identity/queue model.
> Per-call state (the `AESGLOBAL` `global[]` array) is documented in
> [`application.md`](./application.md); the trap/marshalling in
> [`_dispatcher.md`](./_dispatcher.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract:** the *observable* facts — the ap_id space (small integers; 0/1
> system, 2+ apps/DAs), `ap_count = 1` reported, the 8-char space-padded name
> format, the 16-byte standard message, the `-1` not-found convention.
> **Reference (NewTOS reimplements freely):** the `AESPD`/`EVB`/`QPB` structs,
> the ready/deferred/not-ready run lists, `dsptch`/`ev_block`, and the fixed
> `p_queue[128]` layout — these are the EmuTOS cooperative-scheduler internals,
> not something a program observes.

---

## 1. Process descriptors and the run lists

The AES models each application (and each desk accessory) as an **`AESPD`**
process descriptor (`aes/struct.h:119-155`). At start-up
(`geminit.c:810-877`):

- `num_accs = count_accs()` desk accessories are counted (`geminit.c:810`);
- `totpds = num_accs + 2` descriptors exist — pids **0 and 1 are the two AES
  system processes** (the desktop/ctlmgr internals), pids **2 … totpds-1** are
  desk accessories / the running application (`geminit.c:819`,
  `struct.h:36` `NUM_PDS = NUM_ACCS + 2`);
- `curpid` hands out pids sequentially (`gempd.c:60-70`, `geminit.c:875-877`).

The descriptors are held inside the single AES global area `THEGLO D`
(`geminit.c:141`, `aes/gemlib.h:118-144`): `D.g_int[2]` for the two system
processes and `D.g_acc[num_accs]` for accessories (`gemlib.h:118,143`).
`pd_index(i)` maps an index to its descriptor (`gempd.c:32-35`).

**Run-list roots** (`aes/aesvars.h:17-20`, `geminit.c:131`):

| Global | Role |
|--------|------|
| `rlr` | **run-list root** — the currently running AES process descriptor |
| `drl` | deferred/ready list |
| `nrl` | not-ready list (blocked processes) |
| `eul` | free `EVB` list; `dlr`/`zlr` deferred/zombie event lists |
| `indisp` | non-zero while inside `dsptch` (`aesvars.h:38`) |

`rlr` is what `appl_init`/`appl_exit` read and write (`rlr->p_pid`,
`rlr->p_flags`). NewTOS's `rlr` equivalent is "the current AES process".

---

## 2. Application id (`ap_id`)

The application id returned by `appl_init` and stored in `global[2]` is simply
`AESPD.p_pid` (`gemsuper.c:94`, `struct.h:129`). Properties (contract):

- Small integers; **0 and 1 are system**, DAs/apps are **2+**.
- `ap_count` reported by `appl_init` is hard-coded **1** (`gemsuper.c:93`) even
  though EmuTOS runs several AES processes — the *reported* count is 1.
- `appl_find` resolves an 8-char name → pid (or -1); `appl_read`/`appl_write`
  resolve a pid → descriptor via `fpdnm(NULL, pid)` (`gempd.c:38-57`).

**Name format** (`gempd.c:77-86`, `p_nameit`): the descriptor's `p_name`
(`AP_NAMELEN = 8`, `struct.h:111,125`) is the first ≤8 filename chars, stopping
at the first `.` (extension stripped), **space-padded to 8** with `' '`.
`appl_find` compares all 8 bytes with `strncmp` (`gempd.c:48`).

---

## 3. The message pipe (16-byte standard message)

Each descriptor owns a fixed queue: `p_queue[QUEUE_SIZE]` with
`QUEUE_SIZE = 128` (`struct.h:57,152`), a cursor `p_qaddr` and byte count
`p_qindex` (`struct.h:150-151`; initialised `p_qaddr = p_queue`,
`p_qindex = 0` at start-up, `geminit.c:864-865`).

- **`appl_write` (`MU_SDMSG`)** appends a message to the *target's* queue;
  **`appl_read` (`MU_MESAG`)** removes one from the *caller's* queue
  (`gemaplib.c:69-89`, `gemqueue.c:aqueue`/`doq`).
- The **GEM standard message is 16 bytes** = a `WORD msg[8]` array; `msg[0]` is
  the message id (`MN_SELECTED`/`WM_*`/`AC_*`, `include/aesdefs.h:227-243`),
  `msg[1]` = sender ap_id, `msg[2]` = extra bytes, `msg[3..7]` message-specific.
- `ap_rdwr` has a fast 16-byte path (no blocking) when the queue holds exactly
  16 bytes; otherwise it blocks on `ev_block` (`gemaplib.c:77-88`).
- The same pipe backs `evnt_mesag`/`evnt_multi` (`gemevlib.c:116`) — the event
  and application managers share it. See [`event.md`](./event.md).

`doq` coalesces redraw messages (unions `WM_REDRAW` rects for the same window,
overwrites pending `WM_ARROWED`) — a reference-level pipe optimisation
(`gemqueue.c:44-72`).

---

## 4. Process flags (`p_flags`)

`aes/struct.h:115-117`:

| Flag | Value | Meaning |
|------|------:|---------|
| `AP_OPEN`    | 0x0001 | application is between `appl_init` and `appl_exit` |
| `AP_MESAG`   | 0x0002 | application has waited for a message |
| `AP_ACCLOSE` | 0x0004 | application has seen an `AC_CLOSE` message |

`appl_init` sets `AP_OPEN`; `appl_exit` clears it (`gemaplib.c:60,260`).
`evnt_mesag`/`evnt_multi` set `AP_MESAG`/`AP_ACCLOSE`; `appl_exit` uses
`wait_for_accs(AP_ACCLOSE)` to let DAs drain their close (`gemaplib.c:255`).

---

## 5. Other shared AES globals used by the application manager

| Global | Source | Used by |
|--------|--------|---------|
| `gl_nplanes` | `geminit.c` (screen planes) | `appl_init` → `ap_planes` |
| `rs_trees[DESKTOP]` | resource manager | `appl_init` → `ap_private` (DESKTOP root `ob_spec`) |
| `D` (`THEGLO`) | `geminit.c:141` | `appl_init` → `ap_3resv = &D`; `D.g_valstr` scratch used by `appl_exit` |
| `gl_play`/`gl_recd`/`gl_rbuf`/`gl_rlen` | `gemaplib.c:45-48` | `appl_tplay`/`appl_trecord` tape state |

---

## 6. NewTOS mapping (contract vs. reference)

- **Contract to keep:** ap_id = a small process id (system 0/1, apps 2+);
  `appl_init` reports `ap_count = 1`; the 8-char space-padded name; the 16-byte
  message; `-1` for not-found.
- **Free to reimplement:** the `AESPD`/`EVB`/`QPB` structs, the run-list
  scheduler (`dsptch`/`ev_block`/`mwait`), the fixed 128-byte `p_queue`, and the
  `THEGLO` monolith — NewTOS uses its Phase-06 process/IPC model. The AES
  application *is* a NewTOS process; `appl_init`/`appl_exit` bracket its life as
  `Pexec`/`Pterm` do at GEMDOS level.
- **Single-app vs. multi-app:** EmuTOS's concurrent-DA scheduling is reference;
  NewTOS's cooperative single-AES-app (desktop) bring-up satisfies the reported
  `ap_count = 1` with one process and can add DA concurrency later. Open
  question — see [`application.md`](./application.md) NewTOS notes.
