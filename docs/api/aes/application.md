# AES Application Manager (`appl_*`) — opcodes 10–19

> **PHASE-00b D8, Batch 1 (full-contract refinement).**
> Source: EmuTOS @ `1a7484ab`.
> Primary: `aes/gemaplib.c` (`ap_*`); dispatch arm `aes/gemsuper.c:87-123`
> (`crysbind`); message pipe `aes/gemqueue.c`; process descriptors
> `aes/gempd.c`; opcode + slot macros `include/aesdefs.h`.
>
> Call convention + AESPB layout: [`_dispatcher.md`](./_dispatcher.md).
> Shared AES globals (ap ids, the message pipe, `rlr`/`D`):
> [`_state.md`](./_state.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract:** opcode numbers (10–19); the `int_in`/`addr_in` slot layout of
> each call; `int_out[0]` = return code; the `global[]`/`AESGLOBAL` field
> layout and the exact values `appl_init` writes (`ap_version`, `ap_count`,
> `ap_id`, `ap_private`, `ap_ptree`); the 16-byte standard message; the
> 8-char space-padded application-name format used by `appl_find`; the -1
> "not found" / "unsupported opcode" returns. **Reference:** `rlr`/`AESPD`,
> the `p_queue`/`p_qindex` pipe internals, the `QPB`/`ev_block`/`iasync`
> scheduler machinery, the fork/`EVNTREC` tape internals — NewTOS reimplements
> these on its own process/IPC model.

---

## `AESGLOBAL` — the application's AES state (`global[]`)

`aes/gemrslib.h:15-27` (struct) + `include/aesdefs.h:147-157` (the `AP_*`
`global[]` slot macros used by the dispatcher). The `global` pointer of the
AESPB points at this array; `appl_init` fills the first five (public) fields.
Byte offsets are the m68k layout (`WORD` = 2 bytes; `LONG`/pointer = 4 bytes,
2-byte-aligned — no padding):

| Byte | `global[]` word | Field (struct) | Type | Filled by `appl_init` | Meaning |
|-----:|:---------------:|----------------|------|:---------------------:|---------|
| 0x00 | `[0]` `AP_VERSION` | `ap_version` | WORD | yes = `AES_VERSION` | AES version (see below) |
| 0x02 | `[1]` `AP_COUNT`   | `ap_count`   | WORD | yes = 1 | max concurrent applications |
| 0x04 | `[2]` `AP_ID`      | `ap_id`      | WORD | yes = `rlr->p_pid` | this application's id (= AES pid) |
| 0x06 | `[3]` `AP_PRIVATE` | `ap_private` | LONG | yes (see note) | free for the application |
| 0x0A | `[5]` `AP_PTREE`   | `ap_ptree`   | LONG (ptr) | no† | ptr to array of resource-tree addresses |
| 0x0E | `[7]` `AP_1RESV`   | `ap_rscmem`  | LONG (ptr) | no | loaded `.RSC` file in memory (internal) |
| 0x12 | `[9]` `AP_2RESV0`  | `ap_rsclen`  | UWORD | no | `.RSC` length (internal) |
| 0x14 | `[10]` `AP_2RESV1` | `ap_planes`  | WORD | yes = `gl_nplanes` | screen colour planes |
| 0x16 | `[11]` `AP_3RESV`  | `ap_3resv`   | LONG (ptr) | yes = `&D` | ptr to the AES global area `THEGLO` (internal) |
| 0x1A | `[13]` `AP_4RESV`  | `ap_4resv`   | LONG | no | reserved (AES 4.00; unused here) |

> **Correction vs. the previous spec:** the fields at and below `ap_rscmem`
> are *internal* (`/* the following are not advertised to applications */`,
> `gemrslib.h:21`). `appl_init` **does** write `ap_planes` (`AP_2RESV1`,
> `global[10]`, byte 0x14) and `ap_3resv` (`global[11]`, byte 0x16); it does
> **not** write `ap_rscmem`/`ap_rsclen`/`ap_4resv` — those are set later by
> `rsrc_load`. See the exact assignment list under `appl_init`.

> **†`ap_private` / `ap_ptree`:** `appl_init` writes the **`ap_private`** slot
> (`global[3]`) with `rs_trees[DESKTOP][ROOT].ob_spec` (`gemsuper.c:95-96`).
> It does **not** initialise `ap_ptree` (`global[5]`) — that field is populated
> by `rsrc_load` when the application loads its resource file. (The previous
> spec wrongly attributed the DESKTOP `ob_spec` to `ap_ptree`; the source
> writes it to `ap_private`.)
>
> **NewTOS `ap_private` (D-099, closed 2026-09-01, stage S1b).** NewTOS fills
> `ap_private` from the **same object** EmuTOS does: the ROOT of the AES's own
> built-in `DESKTOP` tree, transcribed from `aes/gem_rsc.c:550-570` into
> `tos-api::aes::init` (`desktop_root_obspec()`), read live by `appl_init`.
> Its compiled-in `ob_spec` is the literal `4419L` = **`0x1143`** — the value a
> real TT running EmuTOS reports. At AES boot the `.INF` file's `#Q` line is
> scanned (`process_inf1`, `geminit.c:343-392`) and, if present, its
> plane-appropriate desktop byte replaces the **low byte only**
> (`set_aes_background`, `geminit.c:635-641`), so `ap_private` is
> `0x1100 | patcol`.
>
> An earlier NewTOS revision sourced this from the **WF_NEWDESK-registered**
> desktop-background tree instead. That was wrong: `appl_init` indexes
> `rs_trees[DESKTOP]`, the AES's own compiled-in `gem.rsc` ensemble, while a
> `wind_set(WF_NEWDESK)` tree lands in `gl_newdesk` (`gemwmlib.c:1687`) and is
> read only by `w_drawdesk`/`w_redraw_desktop` — it never reaches `ap_private`.
> The desktop reads `#Q` a **second**, independent time for its own `g_patcol`
> (`desk/deskapp.c:953-960`); two readers of one line is the 1:1 design, not a
> duplication to collapse.

The application's identity is its **AES process id** (`AESPD.p_pid`,
`aes/struct.h:129`). The AES keeps a per-application process descriptor
(`AESPD`, `aes/struct.h:119-155`) holding the 8-char name (`p_name`,
offset 0x0C), the message queue (`p_qaddr`/`p_qindex`/`p_queue[128]`,
`:150-152`), the process link (`p_link`), and flags (`p_flags`, `:136`):
`AP_OPEN = 0x0001` ("between `appl_init` and `appl_exit`"),
`AP_MESAG = 0x0002`, `AP_ACCLOSE = 0x0004` (`aes/struct.h:115-117`).

---

## `AES_VERSION` (the value `appl_init` reports)

`include/config.h:1345-1355`. `AES_VERSION` is **build-configuration
dependent**:

| Value | AES | Selected when |
|------:|-----|---------------|
| `0x0340` | 3.40 | `CONF_WITH_3D_OBJECTS` **and** `CONF_WITH_MENU_EXTENSION` **and** `CONF_WITH_WINDOW_COLOURS` **and** `CONF_WITH_GRAF_MOUSE_EXTENSION` |
| `0x0330` | 3.30 | the last three of the above (no 3D objects) |
| `0x0320` | 3.20 | only `CONF_WITH_GRAF_MOUSE_EXTENSION` |
| `0x0140` | 1.40 | none of the above |

In the **default EmuTOS build** all four flags are `1`
(`include/config.h:1259,1283,1305,1329`), so **`AES_VERSION = 0x0340`**. This
is the value a conforming NewTOS should report from `appl_init` unless a
deliberate ADR pins a different AES level. (Contract note: the *value* is
observable via `global[0]`; the exact figure is a NewTOS decision recorded in
the config notes below, but 0x0340 matches the default reference build.)

---

## `appl_init` (10)

**Source:** `aes/gemsuper.c:90-100` (the `APPL_INIT` arm fills `global[]`
inline) + `aes/gemaplib.c:54-63` (`ap_init` sets the flag and returns the pid).

| Slot | Meaning |
|------|---------|
| `int_in` | (none) |
| `addr_in` | (none) |
| `int_out[0]` | return: application id (`ap_id`), or `-1` on failure |
| `global[]` | filled — see below |

**Behaviour.** The `APPL_INIT` arm writes, in order (`gemsuper.c:92-98`):

- `AP_VERSION` (`global[0]`) = `AES_VERSION` (0x0340 default);
- `AP_COUNT` (`global[1]`) = `1` (the reported max concurrent-application
  count — see the single-app note);
- `AP_ID` (`global[2]`) = `rlr->p_pid` (the running process's pid);
- `AP_PRIVATE` (`global[3]`) = `rs_trees[DESKTOP][ROOT].ob_spec` (the DESKTOP
  resource tree's root `ob_spec`, the desktop background pattern/colour word) —
  **NewTOS:** the same object, from the built-in `DESKTOP` tree in
  `tos-api::aes::init` (`desktop_root_obspec()`): `0x1143`, or `0x1100 | patcol`
  after a `#Q` line (see the `ap_private` callout above and D-099);
- `AP_2RESV1`/`ap_planes` (`global[10]`) = `gl_nplanes` (screen colour planes) —
  **NewTOS:** log2 of the framebuffer colour count (`screen_planes()`);
- `AP_3RESV`/`ap_3resv` (`global[11]`) = `&D` (address of the AES global area
  `THEGLO`, an internal pointer) — **NewTOS:** the AES process-descriptor table
  address (`tos-api::aes::pd::table_addr()`), the process-management core of our
  AES state. Written as a **4-byte** field at byte 0x16 (no gemlib reader; kept
  4 bytes so the app-readable `ap_maxchar`/`ap_minchar` at `global[13]`/`[14]`
  stay 0 as at EmuTOS `appl_init`; on LP64 the 64-bit pointer is truncated —
  harmless, nothing reads it back).

Then `ap_init()` (`gemaplib.c:54`) sets `rlr->p_flags |= AP_OPEN` and returns
`rlr->p_pid`. That pid becomes `RET_CODE` (`int_out[0]`).

`rlr` = "run-list root" = the currently running AES process descriptor
(`aes/aesvars.h`; see [`_state.md`](./_state.md)).

> `appl_init` never actually fails in this EmuTOS build — `ap_init` always
> returns a valid pid (the process descriptor was allocated at AES start-up).
> The documented `-1` failure return is the contract for a NewTOS that *can*
> fail to allocate an application slot.

## `appl_read` (11) / `appl_write` (12)

**Source:** `aes/gemsuper.c:101-105` → `aes/gemaplib.c:69-89` (`ap_rdwr`);
pipe mechanics `aes/gemqueue.c` (`aqueue`/`doq`). The inter-application
**message pipe**.

| Slot | Name | Meaning |
|------|------|---------|
| `int_in[0]` | `AP_RWID` | target/own application id (the *pid*) |
| `int_in[1]` | `AP_LENGTH` | byte count |
| `addr_in[0]` | `AP_PBUFF` | message buffer (`WORD *`) |
| `int_out[0]` | — | return: non-zero (1) on success |

**Behaviour.** The dispatcher resolves the id to an `AESPD` via
`fpdnm(NULL, AP_RWID)` (find process descriptor by pid, `gempd.c:38-57`), then
calls `ap_rdwr(code, p, AP_LENGTH, AP_PBUFF)` with `code = MU_MESAG` for
`appl_read` and `code = MU_SDMSG` for `appl_write` (`gemsuper.c:103-104`).

- **`MU_MESAG` (0x0010, `include/aesdefs.h:260`)** = read from *its own* pipe.
- **`MU_SDMSG` (0x0040, `aes/gemlib.h:25`)** = send to *another* app's pipe.

`ap_rdwr` (`gemaplib.c:69`) has a **fast path**: when `code == MU_MESAG`, the
pipe currently holds exactly `p->p_qindex == length == 16` bytes, it `memcpy`s
16 bytes to the buffer, clears `p_qindex`, and returns `1` (`gemaplib.c:77-82`).
Otherwise it builds a `QPB` (`qpb_ppd`/`qpb_cnt`/`qpb_buf`, `aes/struct.h:158`)
and **blocks** via `ev_block(code, &m)` (`gemaplib.c:84-88`) until the queue
operation completes (`aqueue`/`doq` in `gemqueue.c`). The **16-byte** message
is the GEM standard — the `msg[8]` `WORD` array delivered by `evnt_mesag`
(which itself calls `ap_rdwr(MU_MESAG, rlr, 16, mebuff)`, `gemevlib.c:116`).

> **Redraw coalescing (source detail):** on a *write* of a `WM_REDRAW`/
> `WM_ARROWED`/`WM_HSLID`/`WM_VSLID` message, `doq` unions the redraw rectangle
> with a pending matching-handle `WM_REDRAW` already in the queue, or overwrites
> a pending `WM_ARROWED` (`gemqueue.c:44-72`). This is a reference-level
> optimisation of the pipe, not a slot-level contract.

## `appl_find` (13)

**Source:** `aes/gemsuper.c:106-108` → `aes/gemaplib.c:95-108` (`ap_find`);
lookup `aes/gempd.c:38-57` (`fpdnm`).

| Slot | Name | Meaning |
|------|------|---------|
| `addr_in[0]` | `AP_PNAME` | 8-char application name (`char *`) |
| `int_out[0]` | — | return: the app's pid, or `-1` if not found / `pname == NULL` |

**Behaviour.** `ap_find` first rejects a `NULL` name pointer with `-1`
(`gemaplib.c:103-104`) — an explicit guard, because `fpdnm(NULL, …)` means
"search by pid" instead of by name. Otherwise it calls `fpdnm(pname, 0)`, which
walks all `totpds` process descriptors (`gempd.c:43`) and compares the name
with `strncmp(pname, p->p_name, AP_NAMELEN)` over **exactly 8 characters**
(`gempd.c:48`, `AP_NAMELEN = 8`, `aes/struct.h:111`). It returns the matched
`p->p_pid`, or `-1` if no descriptor matches.

**Name format (contract).** Names in the descriptor are set by `p_nameit`
(`gempd.c:77-86`): copy up to 8 characters of the filename, **stopping at the
first `.`** (extension stripped), then **space-pad to 8** with `' '` (0x20).
So `appl_find` must be passed the 8-char, space-padded, extension-less name
(e.g. `"CONTROL "` for `CONTROL.ACC`); a shorter string only matches if the
caller space-pads it, because `strncmp` compares the full 8 bytes.

## `appl_yield` (17)

**Source:** `aes/gemsuper.c:115-119` → `dsptch()` (the AES scheduler). A
**PC-GEM extension**, compiled only `#if CONF_WITH_PCGEM`
(`gemsuper.c:115`). `CONF_WITH_PCGEM` defaults to **1**
(`include/config.h:1319-1321`), so `appl_yield` is present in the default
build.

| Slot | Meaning |
|------|---------|
| `int_in` / `addr_in` | (none) |
| `int_out[0]` | return: `TRUE` (1) — `ret` is left at its initial `TRUE` (`gemsuper.c:85`) |

Cooperatively yields the CPU to any other ready AES process by calling
`dsptch()` (the run-list dispatcher). If `CONF_WITH_PCGEM` is 0, opcode 17
falls to the `default` arm and returns `-1` (see [`_dispatcher.md`](./_dispatcher.md)).

## `appl_exit` (19)

**Source:** `aes/gemsuper.c:120-123` → `aes/gemaplib.c:251-261` (`ap_exit`).

| Slot | Meaning |
|------|---------|
| `int_in` / `addr_in` | (none) |
| `int_out[0]` | return: `TRUE` (1) — `ap_exit` is `void`; `ret` stays `TRUE` |

**Behaviour.** `ap_exit` tears the application down (`gemaplib.c:252-260`):

1. `wm_update(BEG_UPDATE)` — lock the screen against redraws.
2. `mn_cleanup()` — drop the app's menu bar if it owns one.
3. `wait_for_accs(AP_ACCLOSE)` — block until every desk accessory has seen the
   `AC_CLOSE` message (`geminit.c:599-628`; polls the DA descriptors' flags
   with a `WAIT_TIMEOUT = 500` loop cap).
4. If the app still has a queued message (`rlr->p_qindex != 0`), drain it with
   `ap_rdwr(MU_MESAG, rlr, rlr->p_qindex, D.g_valstr)` (`gemaplib.c:256-257`;
   `D.g_valstr` is a scratch buffer in `THEGLO`, `aes/gemlib.h:133`).
5. `wm_update(END_UPDATE)` — unlock the screen.
6. `all_run()` — re-schedule the other processes (`geminit.c:554`).
7. `rlr->p_flags &= ~AP_OPEN` — mark `appl_exit` done.

> `appl_exit` always returns `1`. The DA-close handshake (steps 2-3) presumes
> concurrent AES processes; in NewTOS's single-app bring-up there are no DAs, so
> `wait_for_accs` returns immediately.

## `appl_tplay` (14) / `appl_trecord` (15)

**Source:** `aes/gemsuper.c:109-114` → `aes/gemaplib.c:119-184` (`ap_tplay`) /
`:189-246` (`ap_trecd`). Event-"tape" play/record: replay or capture a stream
of `EVNTREC` records (timer/button/mouse/keyboard events) for macros/testing.

| Slot | Name | Meaning |
|------|------|---------|
| `addr_in[0]` | `AP_TBUFFER` | `EVNTREC *` — the event tape |
| `int_in[0]` | `AP_TLENGTH` | record count |
| `int_in[1]` | `AP_TSCALE` | playback time-scale (`ap_tplay` only) |
| `int_out[0]` | — | `appl_tplay`: `TRUE` (1), it is `void`; `appl_trecord`: actual #records captured |

**`EVNTREC`** (`aes/gemaplib.h:17-28`) is two LONGs: `ap_event` (the event
kind — `TCHNG`=0 timer, `BCHNG`=1 button, `MCHNG`=2 mouse, `KCHNG`=3 keyboard)
and `ap_value` (packed event data; encoding per kind documented at
`gemaplib.h:24-28`).

**`ap_tplay`** iterates the buffer, converting each record into a fork-queued
call (`bchange`/`mchange`/`kchange`) or an `ev_timer` delay scaled by
`AP_TSCALE` (`gemaplib.c:127-168`), and `dsptch()`es between records. Mouse
playback temporarily disconnects the VDI cursor and redraws it itself
(`gemaplib.c:140-153,173-181`).

**`ap_trecd`** sets `gl_recd`/`gl_rbuf`/`gl_rlen` and lets the fork dispatcher
(`gemdisp.c`) record raw `FPD`s into the buffer, polling every 0.1 s until the
buffer fills, then converts the captured `FPD`s back to `EVNTREC` format and
returns the actual count (`gemaplib.c:209-245`). Relies on
`_Static_assert(sizeof(EVNTREC) == sizeof(FPD))` (`gemaplib.c:206`).

> **NewTOS note (honest-minimal).** These two need the fork-queue + input-event
> recorder infrastructure (`gemdisp.c`, `forkq`, `gl_recd`). NewTOS has no such
> recorder at bring-up. Per CLAUDE.md Rule 2 (no stubs), `appl_tplay`/
> `appl_trecord` are **honest-unsupported** until the input subsystem exists:
> the dispatch arms may return `-1` (or the documented `TRUE`/count once the
> recorder is built), but must not silently pretend to replay/capture. The slot
> ABI above is the contract for when they are implemented.

---

## Trap interface (parameter-block form)

All `appl_*` calls enter via the GEM trap (`svc #2`) with **selector 200 in
`x0`** and an **AESPB pointer in `x1`** — see [`_dispatcher.md`](./_dispatcher.md).
Per call, the dispatcher reads `control[0]` = opcode, `control[1]` = #int_in,
`control[2]` = #int_out, `control[3]` = #addr_in, and returns via
`int_out[0]` = `RET_CODE`.

| Opcode | Function | `int_in` | `addr_in` | `int_out[0]` return |
|-------:|----------|----------|-----------|---------------------|
| 10 | `appl_init`   | — | — | `ap_id` (pid) / -1; fills `global[]` |
| 11 | `appl_read`   | `[0]`=id, `[1]`=len | `[0]`=buf | 1 on success |
| 12 | `appl_write`  | `[0]`=id, `[1]`=len | `[0]`=buf | 1 on success |
| 13 | `appl_find`   | — | `[0]`=name | pid / -1 |
| 14 | `appl_tplay`  | `[0]`=len, `[1]`=scale | `[0]`=`EVNTREC*` | 1 (void) |
| 15 | `appl_trecord`| `[0]`=len | `[0]`=`EVNTREC*` | #records captured |
| 17 | `appl_yield`  | — | — | 1 *(CONF_WITH_PCGEM; else -1)* |
| 19 | `appl_exit`   | — | — | 1 |

> **Not present in this build:** `appl_getinfo` (130), `appl_search` (18),
> `appl_control`. No opcode macro and no dispatch arm exists for any of them in
> EmuTOS `1a7484ab` (`include/aesdefs.h:16-24`, `aes/gemsuper.c:88-123`). Calls
> to those opcodes hit the dispatcher `default` arm → `-1` (see
> [`_dispatcher.md`](./_dispatcher.md)). If NewTOS later adds `appl_getinfo`
> for MagiC/newer-GEM compatibility that is a documented extension, not a
> reference-parity item.

---

## Inputs / Outputs summary

All inputs arrive in the AESPB arrays (no register/stack params). Outputs are
`int_out[0]` (the return code) plus, for `appl_init`, the `global[]` fields
listed above. `appl_read`/`appl_write` also transfer the 16-byte message
through `addr_in[0]`.

---

## Error conditions

The application manager has almost no error surface — it is "trusted" AES
code. The only defined negative returns:

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `appl_find` with `pname == NULL` | `gemaplib.c:103-104` | `-1` |
| `appl_find` name not in any descriptor | `gemaplib.c:106-107` | `-1` |
| `appl_init` cannot allocate an app slot | (contract; never happens here) | `-1` |
| `appl_yield` when `CONF_WITH_PCGEM == 0` | `gemsuper.c:406-417` | `-1` (unsupported opcode) |

There are **no** TOS negative error codes (`E*`) in the application manager;
returns are pid / 0 / 1 / -1 only.

---

## Verification criteria

- **`appl_init` id + version:** `appl_init` returns a **non-negative** value
  (the pid), sets `global[0]` = `AES_VERSION` (0x0340 in the default build),
  `global[1]` = 1, and `global[2]` = the same pid it returned
  (`gemsuper.c:92-99`).
- **`appl_init` private:** after `appl_init`, `global[3..4]` (`ap_private`,
  read via `ULONG_AT(&global[3])`) equals the DESKTOP tree root's `ob_spec`
  (`gemsuper.c:95-96`). **Satisfied on NewTOS** since stage S1b: `0x1143`
  with no `#Q` line in the `.INF` file, `0x1100 | patcol` with one — the high
  24 bits are the resource constant and never change (D-099, closed).
- **`appl_find` unknown:** `appl_find` of a name that no application registered
  returns `-1` (`gemaplib.c:106-107`).
- **`appl_find` NULL:** `appl_find` with `addr_in[0] == NULL` returns `-1`
  without searching (`gemaplib.c:103-104`).
- **`appl_find` self:** `appl_find` of the caller's own 8-char space-padded
  name returns the caller's pid (`gempd.c:48`).
- **16-byte read fast path:** with the pipe holding exactly a 16-byte message,
  `appl_read(id=self, len=16, buf)` returns `1` and copies the 16 bytes
  (`gemaplib.c:77-82`).
- **`appl_exit` return:** `appl_exit` returns `1` and clears `AP_OPEN`
  (`gemaplib.c:260`).
- **`appl_yield` present:** in the default build (`CONF_WITH_PCGEM=1`) opcode
  17 returns `1`; if that flag is 0, opcode 17 returns `-1`.

---

## Edge cases discovered in source

1. **`ap_id` is the AES pid, not a separate handle.** `appl_init` returns
   `rlr->p_pid` and stores the same value in `global[2]`
   (`gemsuper.c:94,99`). The AES pid is small (0/1 = system processes, 2+ =
   accessories/applications, `geminit.c:875-877`).
2. **`ap_private` — not `ap_ptree` — carries the DESKTOP `ob_spec`.**
   `appl_init` writes `global[3]` (`AP_PRIVATE`), *not* `global[5]`
   (`AP_PTREE`). `ap_ptree` stays unset until `rsrc_load` (`gemsuper.c:95-96`).
3. **`appl_init` also sets `ap_planes` and `ap_3resv`** (`global[10]` and
   `global[11]`) even though those are "not advertised" fields — a program that
   reads them sees the screen plane count and the `THEGLO` pointer
   (`gemsuper.c:97-98`).
4. **`appl_find` compares exactly 8 bytes.** A caller passing a name shorter
   than 8 chars must space-pad it, or `strncmp` over 8 bytes will mismatch
   against the space-padded descriptor name (`gempd.c:48,77-86`).
5. **`appl_find(NULL)` is a defined `-1`, not a crash**, because a NULL name has
   the special "search by pid" meaning inside `fpdnm` and is explicitly guarded
   (`gemaplib.c:99-104`).
6. **`appl_read`/`appl_write` block the AES process** unless the 16-byte fast
   path applies; blocking is via the cooperative AES scheduler (`ev_block` →
   `mwait`/`dsptch`), not a preemptive wait (`gemaplib.c:84-88`,
   `gemevlib.c:64-70`).
7. **`appl_write` to self is legal** — send id == own id delivers into the
   caller's own pipe (the id is resolved with `fpdnm(NULL, id)`, no self-check).
8. **The dispatcher's `default` arm returns `-1` and shows an alert.** Opcodes
   16, 18, and (without PC-GEM) 17 within the 10–19 range are not handled and
   fall through to `fm_show(ALNOFUNC, …)` + `ret = -1`, except opcode 0 which is
   silently ignored (`gemsuper.c:406-417`). See [`_dispatcher.md`](./_dispatcher.md).
9. **`appl_exit` may drain a leftover message** into `D.g_valstr` before
   returning, so a queued-but-unread message at exit is consumed, not leaked
   (`gemaplib.c:256-257`).

---

## Dependencies

### Requires (must be initialized / functional)

- The AES process model: `rlr`, the `AESPD` descriptors, `totpds`/`curpid`
  (`geminit.c` start-up; [`_state.md`](./_state.md)). `appl_init` needs a
  descriptor already allocated.
- `rs_trees[DESKTOP]` (the built-in DESKTOP resource) for `appl_init`'s
  `ap_private` value (`gemsuper.c:95`).
- The AES scheduler (`ev_block`/`mwait`/`dsptch`) and the message pipe
  (`gemqueue.c`) for `appl_read`/`appl_write`.
- `wm_update`, `mn_cleanup`, `wait_for_accs`, `all_run` for `appl_exit`.
- The fork/input-recorder subsystem (`gemdisp.c`) for `appl_tplay`/
  `appl_trecord` — **absent at NewTOS bring-up**.

### Used by

- **Every GEM application** brackets its life with `appl_init` … `appl_exit`.
- `evnt_mesag` / `evnt_multi` (event manager) sit on the same 16-byte message
  pipe as `appl_read`/`appl_write` (`gemevlib.c:116`) — see
  [`event.md`](./event.md).
- `mn_register` and the desk-accessory machinery use `appl_find`/the pid model.

---

## Related data structures

- **`AESGLOBAL`** — the `global[]` array, layout table above
  (`gemrslib.h:15-27`).
- **16-byte standard message** — `WORD msg[8]`; `msg[0]` = message id
  (`MN_SELECTED`/`WM_*`/`AC_*`, `include/aesdefs.h:227-243`), `msg[1]` = sender
  ap_id, `msg[2]` = extra-byte count, `msg[3..7]` = message-specific. Shared
  with the event manager ([`event.md`](./event.md)).
- **`EVNTREC`** (tape record) — `gemaplib.h:17-28`.
- **`AESPD` / `QPB`** — process descriptor + queue parameter block
  (`aes/struct.h:119-163`); reference-level (NewTOS reimplements). See
  [`_state.md`](./_state.md).
- **`appl_find` name format** — 8 chars, extension stripped at first `.`,
  space-padded (`gempd.c:77-86`).

---

## Cross-references

### Atari Compendium / Profibuch

- **Not present in `references/`** (no PDFs). Cross-check of `appl_init`'s
  reported `ap_version`/`ap_count`, the `appl_find` 8-char/space-pad format and
  the `-1` conventions, the `appl_read`/`appl_write` 16-byte pipe, and the
  absence of `appl_getinfo`/`appl_search` in this build is **deferred** until
  the documentation PDFs are available.

### AES dispatcher

- [`_dispatcher.md`](./_dispatcher.md) — the selector-200 call model, `control[]`
  counts, `int_out[0]` return convention, opcode→manager map, and the
  `default` → `-1` unsupported-opcode path.

---

## Configuration / version notes

- **`AES_VERSION`** reported by `appl_init` is build-dependent (table above);
  **0x0340** in the default reference build. NewTOS should pin the reported
  value by ADR; matching the default (0x0340) keeps parity with EmuTOS's
  standard build.
- **`CONF_WITH_PCGEM`** (default **1**) gates `appl_yield` (17). With it off,
  opcode 17 is an unsupported opcode (`-1`).
- **`ap_count = 1`** is hard-coded (`gemsuper.c:93`) — EmuTOS reports a single
  concurrent application even though it runs desk accessories as additional AES
  processes. NewTOS's single-AES-app (desktop) model matches this reported
  value naturally.
- The tape functions (`appl_tplay`/`appl_trecord`) are configuration-
  independent in EmuTOS but depend on the input/fork subsystem NewTOS has not
  built.

---

## NewTOS implementation notes

(Empty during Phase 00.)

- **`ap_private` — the AES's own built-in `DESKTOP` tree (D-099, closed
  2026-09-01, stage S1b).** `tos-api::aes::init` carries `rs_trees[DESKTOP]`
  as a static three-object tree transcribed from `aes/gem_rsc.c:550-570` (ROOT
  `ob_spec = 4419`, the background box, and the `DTNAME` `G_TEXT` with its
  `rs_tedinfo[12]`). AES boot (`aes::init::init()`, called from the kernel boot
  path before anything can `appl_init`) does the three `geminit.c` steps in
  EmuTOS's order: `process_inf1`'s `#Q` scan of the first `INF_SIZE` = 300 bytes
  of `C:\NEWDESK.INF` (`geminit.c:343-392`, `:99`), the `gem_rsc_fixit`
  char-grid→pixel conversion plus `tree[ROOT].ob_height = screen height`
  (`gem_rsc.c:636-641`, `geminit.c:715-717`), and `set_aes_background`
  (`geminit.c:635-641`) when a `#Q` line was found. `appl_init` then reads
  `rs_trees[DESKTOP][ROOT].ob_spec` verbatim (`gemsuper.c:95-96`).
  The earlier "no DESKTOP tree exists at the AES layer, so report 0" position
  is withdrawn: the tree is the AES's own resource, not the desktop's, so
  nothing about it needed the desktop client.
- **Single-app vs. multi-app (the contract split).** EmuTOS runs the desktop
  plus up to `NUM_ACCS` desk accessories as concurrent AES processes, but
  reports `ap_count = 1`. The **reported value 1** and the **pid-as-ap_id**
  model are *contract*; the concurrent-DA scheduling is *reference*. NewTOS's
  cooperative single-AES-app (desktop) bring-up satisfies the contract with one
  process; DA concurrency can be added later without changing what `appl_init`
  reports. Open question flagged in the report.
- **`ap_id` = the Phase-06 process id** — the AES application is a process;
  `appl_init`/`appl_exit` bracket its life exactly as `Pexec`/`Pterm` do at the
  GEMDOS level.
- **The message pipe** maps onto NewTOS's process/IPC model; the 16-byte
  standard message is what `evnt_mesag` delivers — the two are designed
  together (see [`event.md`](./event.md)).
- **`global[]` is an application-supplied array** (in the app's memory) — a
  **user pointer**; apply the same boundary handling as other AES `addr*`
  pointers (see [`_dispatcher.md`](./_dispatcher.md) §NewTOS notes). The
  m68k-vs-native pointer-width variance of `ap_ptree`/`ap_private` follows the
  ABI of the caller (see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md)
  "Native vs m68k struct variants").
- **`appl_tplay`/`appl_trecord`** are honest-unsupported until the input
  recorder exists (Rule 2), not stubbed.
