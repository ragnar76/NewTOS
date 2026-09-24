# Desk Accessories — the full `.ACC` lifecycle (ACC01)

> Source: EmuTOS @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df` (pinned commit,
> verified present in `references/emutos/` at analysis time, 2026-09-24).
> Primary files: `aes/geminit.c`, `aes/gemmnlib.c`, `aes/gemctrl.c`,
> `aes/gemwmlib.c`, `aes/gemaplib.c`, `aes/gemshlib.c`, `aes/gempd.c`,
> `aes/gemasm.S`, `bios/bios.c`, `bios/blkdev.c`, `bdos/fsdir.c`,
> `bdos/kpgmld.c`, `include/sysconf.h`, `include/biosext.h`, `include/config.h`.
>
> **This file is the end-to-end lifecycle narrative ACC01 asks for.** Several
> pieces of it are already extracted in depth elsewhere in `docs/api/aes/` —
> `menu.md` (menu-bar/DA-slot mechanics, `mn_register`/`menu_fixup`/`AC_OPEN`
> message layout), `shell-loop.md` (`gem_main`/AESPD numbering/`sh_ldapp`),
> `application.md` (`appl_exit`/`appl_init`), `event.md` (`AP_MESAG`/
> `AP_ACCLOSE` flag-setting), `_state.md` (the DA-slot globals table). This
> file does not re-derive those; it cites them and adds what was not written
> down anywhere yet: the **load-time** half of the lifecycle
> (`count_accs`/`load_one_acc`/`load_accs`/`free_accs`/`run_accs_and_desktop`),
> the boot-flag effects, the gating check, the extension-family cost analysis,
> and the `sysacc.c` contract review — all things ACC01 was explicitly asked
> for and none of the existing files cover.
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> What an application/accessory can observe (the `AC_OPEN`/`AC_CLOSE` message
> words, the `menu_register` return value, the boot-flag key, the load order
> relative to the desktop) is **contract**. The mechanism EmuTOS uses to
> produce it (the m68k `gotopgm`/`psetup` process-entry trampoline, the exact
> shape of the `ACC`/`AESPROCESS` structs) is **reference** — NewTOS may
> implement the mechanism differently as long as the observable behaviour
> matches.

---

## 0. Ticket-citation audit

ACC01 names most functions with line ranges as a "starting map, not verified
fact." Checked against the pinned source:

| Ticket citation | Verified | Actual |
|---|---|---|
| `count_accs` `geminit.c:265-284` | **off by one** | function starts at `:264` (`static WORD count_accs(void)`), body through `:284` |
| `load_one_acc` `geminit.c:236-257` | **correct** | `:236-257` exactly |
| `load_accs` `geminit.c:307-315` | **off** | `:307-308` is the function's doc-comment; the function itself is `:309-315` |
| `free_accs` `geminit.c:288-303` | **off** | `:288-295` is the doc-comment (which also documents *why* — Chameleon needing explicit free); the function itself is `:296-303` |
| `run_accs_and_desktop` `geminit.c:654-745` | **short by 5** | the function's closing brace (after `gsx_wsclose()`) is at `:750`, not `:745` — `:745` lands mid-body, one line after the `sh_main` call |
| `wait_for_accs` `geminit.c:588-630` | **off** | `:588` is inside the function's doc-comment (the `AP_ACCLOSE` bullet); the function signature is at `:599`, body through `:628`, blank line to `:630` |
| `BOOTFLAG_SKIP_AUTO_ACC` `geminit.c:268-270` | **off by one** | the check is `:269-270`; `:268` is a one-line comment above it |
| `AC_OPEN` poster `gemctrl.c:337-346` | **correct** | the `title == THEDESK` branch and `mesag = AC_OPEN;` line are exactly `:337-346` |
| `AC_OPEN` poster (via) `gemwmlib.c:851-862` | **correct** | `ap_sendmsg`'s signature through its `ap_rdwr` call is exactly `:851-862` (the closing brace is `:863`, outside the cited range, which is fine — the range covers the whole meaningful body) |
| `mn_cleanup` `gemmnlib.c:740-760` | **long by 2** | the function is `:740-758`; `:759-760` are blank lines before the next function |
| `mn_cleanup` AC_CLOSE line `gemmnlib.c:747` | **correct** | `ap_sendmsg(appl_msg, AC_CLOSE, desk_ppd[i], i, 0, 0, 0, 0);` is exactly `:747` |
| `ap_exit` caller `gemaplib.c:254` | **off by one** | `:254` is the `mn_cleanup();` call inside `ap_exit`; the `wait_for_accs(AP_ACCLOSE)` call the ticket is pointing at is `:255` |
| `sh_ldapp` caller `gemshlib.c:652` | **off by one** | `:652` is a `KDEBUG` line; the `mn_cleanup();` call is `:653` |

None of these are wrong-function citations — every one points at the right
neighbourhood, and several (`load_one_acc`, `gemctrl.c:337-346`,
`gemwmlib.c:851-862`, `gemmnlib.c:747`) are exactly right. The drift is
consistently "off by the doc-comment/blank-line boundary," never a
misidentification. Corrected line numbers are used throughout this document.

---

## 1. The state machine, in words

Six distinct phases, in the order EmuTOS actually runs them:

1. **Discovery** (`gem_main`, once per boot/rez-change, before any AESPD
   exists) — `count_accs()` walks the root directory for up to `NUM_ACCS = 6`
   (`include/sysconf.h:37-40`) files matching `\*.ACC`, recording their names
   only (`geminit.c:264-284`). Nothing is loaded yet.
2. **Slot reservation** (`gem_main`, `geminit.c:812-877`) — an `AESPROCESS`
   array sized to the discovered count is allocated (`num_accs *
   sizeof(AESPROCESS)`), `totpds = num_accs + 2` (`:819`), and the AESPD table
   is initialised top-down for **all** `totpds` slots — slot 0 (the "dos
   appl" slot `rlr` stays pointed at forever), slot 1 (reserved for
   `SCRENMGR`), slots 2..`num_accs+1` (reserved for the accessories, in
   `count_accs`'s discovery order). This happens before a single accessory
   process exists — the slots are pre-numbered, not assigned as each
   accessory starts.
3. **Load and start** (`run_accs_and_desktop`, `geminit.c:687` →
   `load_accs(num_accs)` → `load_one_acc` per file, `:236-257`) — each `.ACC`
   file is `dos_open`ed, its program header + relocations are read via
   `pgmld`/`kpgmld` (`bdos/kpgmld.c:40` onward — the same GEMDOS program-header
   loader every `.PRG`/`.APP`/`.TOS`/`.TTP` uses), and a process is created for
   it with `pstart(gotopgm, acc->name, acc->addr)` (`:255`). `pstart` →
   `getpd()` consumes the next `curpid` slot (2, 3, … in file-discovery
   order — see [`shell-loop.md`](./shell-loop.md) §4 for the full AESPD
   numbering derivation) and calls `psetup` to arrange that when this process
   is next scheduled, it starts executing at `gotopgm`
   (`aes/gemasm.S:73-131`) — the m68k-specific trampoline that switches to
   user mode at the loaded program's entry point (`aes/gemasm.S:83-110`; see
   §5 below on why this step is architecture-bound).
4. **First run and synchronisation** (`run_accs_and_desktop`, `:736-737`) —
   `dsptch()` starts the scheduler; `wait_for_accs(AP_MESAG)`
   (`geminit.c:599-628`) then blocks (polling `all_run()` in a loop capped at
   `WAIT_TIMEOUT = 500` iterations, `:609`) until every accessory AESPD has
   `p_flags & AP_MESAG` set — i.e. until each accessory has made its first
   `evnt_mesag()`/`evnt_multi()` call. This is the point at which the AES
   trusts that every accessory has finished its own `appl_init` +
   `menu_register` sequence and is sitting in its own message loop, so any
   VDI workstation memory it allocated during init becomes owned by the
   (long-lived) AES rather than by whatever program launches next.
5. **Steady state** (`sh_main`'s loop, documented in
   [`shell-loop.md`](./shell-loop.md) §7-8) — accessories run cooperatively
   alongside the desktop and every launched application via `dsptch()`. Each
   accessory registered its menu title during phase 4 via `menu_register`
   (AES 35, `gemmnlib.c:789-816`), which spliced its title into the `THEDESK`
   drop-down (`menu_fixup`, `gemmnlib.c:89-125` — see
   [`menu.md`](./menu.md) for the full mechanics). Picking that title sends
   `AC_OPEN`; every program termination sends `AC_CLOSE` to every registered
   accessory (§2-3 below).
6. **Teardown** (`run_accs_and_desktop`, `:741` — only on shutdown or
   resolution change, i.e. when the *outer* `accdesk_start` process itself
   ends, not per-application) — `free_accs(num_accs)` (`geminit.c:296-303`)
   frees each accessory's memory via `dos_free`. The comment at `:290-295`
   is explicit that this is **not required for correctness** (process
   termination would free it anyway) but is done because at least one
   real-world accessory (Chameleon) needs the explicit free to trigger its
   own cleanup.

An accessory process itself never leaves step 5 under normal operation — it
has no `appl_exit`/`Pterm` call in its own code (§4 below); the only way an
accessory process actually stops running is the whole AES session ending at
step 6.

---

## 2. The boot flag — `BOOTFLAG_SKIP_AUTO_ACC`

**Source:** `include/biosext.h:38-42` (bit definitions), `bios/bios.c:816-822`
(where it is set), `bios/bios.c:702-703`, `bios/blkdev.c:236-239`,
`aes/geminit.c:269-270`, `:452`, `:766-767` (where it is checked).

- **Bit:** `BOOTFLAG_SKIP_AUTO_ACC = 0x04` (`biosext.h:42`), one of three
  boot flags in the global `UBYTE bootflags` (`BOOTFLAG_EARLY_CLI = 0x01`,
  `BOOTFLAG_SKIP_HDD_BOOT = 0x02` from `MODE_ALT`).
- **Which key:** `biosmain()` samples `shiftbits` (from `kbshift(-1)`, or
  from the welcome screen if `show_initinfo` is true) and sets the flag if
  `shiftbits & MODE_CTRL` (`bios.c:819-820`) — **holding Control** at boot.
  `MODE_CTRL = 0x04` (`include/biosdefs.h:92`).
- **What it skips**, despite the name only naming "AUTO_ACC":
  1. **Floppy bootsector execution.** `blkdev_boot()` returns immediately
     without running the bootsector code (`blkdev.c:236-239`).
  2. **`\AUTO\*.PRG` execution.** `autoexec()` returns immediately
     (`bios.c:702-703`), skipping both `bootstrap()` (native-features OS
     chainload, `DETECT_NATIVE_FEATURES` builds only) and the `\AUTO\` scan.
  3. **Desk accessory discovery.** `count_accs()` returns `0` without
     touching the filesystem at all (`geminit.c:269-270`) — no `.ACC` files
     are even *counted*, so nothing downstream (slot reservation, load,
     `menu_register`) has anything to act on.
  4. **`EMUDESK.INF` is not read.** `gem_main` skips the `readfile` call
     entirely (`n = 0`, `geminit.c:766-767`) — so `process_inf1`/
     `process_inf2` see an empty buffer: no `#E` resolution change, no `#Q`
     background, and (functionally, since the buffer is empty) no `#Z`
     autorun. `process_inf2` also has its own **independent** guard around
     the `sh_wdef` autorun-program registration (`:452`,
     `if (!(bootflags & BOOTFLAG_SKIP_AUTO_ACC))`) — redundant given point 4
     already empties the buffer, but present as a second, explicit check on
     the same flag.
- **Net effect:** holding Control at boot produces a "clean" EmuTOS session —
  no bootsector, no `\AUTO\` programs, no accessories, no saved desktop
  layout — a diagnostic/recovery mode, not an accessory-specific toggle.

---

## 3. The `AC_OPEN`/`AC_CLOSE` message-word asymmetry — **confirmed**

The ticket's claim ("`msg[4]` for `AC_OPEN`, `msg[3]` for `AC_CLOSE`") is
**correct**, traced end to end through `ap_sendmsg`'s fixed argument order
rather than assumed:

`ap_sendmsg(WORD ap_msg[], WORD type, AESPD *towhom, WORD w3, WORD w4, WORD
w5, WORD w6, WORD w7)` (`aes/gemwmlib.c:851-862`) packs unconditionally:
```c
ap_msg[0] = type;
ap_msg[1] = rlr->p_pid;   /* sender's pid, not the recipient's */
ap_msg[2] = 0;
ap_msg[3] = w3;
ap_msg[4] = w4;
ap_msg[5] = w5;  ap_msg[6] = w6;  ap_msg[7] = w7;
```

**`AC_OPEN`** — `gemctrl.c:337-350`'s `hctl_rect()`, on a click at or past
`gl_dafirst` in the `THEDESK` drop-down:
```c
mesag = AC_OPEN;
...
mn_getownid(&owner, &item, item);   /* resolves display-slot -> (owner pd, menu-id) */
...
ct_msgup(mesag, owner, title, item, treehi, treelo, parent);
```
`ct_msgup(message, owner, wh, m1, m2, m3, m4)` (`gemctrl.c:81-102`) calls
`ap_sendmsg(appl_msg, message, owner, wh, m1, m2, m3, m4)` — so **`w3 = wh =
title`** (the menu title object, i.e. `THEDESK` — a constant, since every
accessory sits under the same title) and **`w4 = m1 = item`** (the
accessory's own menu id, resolved by `mn_getownid` from the clicked display
slot via `acc_display[]`, `gemmnlib.c:838-848`). Result: **`msg[3] =
THEDESK`, `msg[4] =` the accessory's menu id.**

**`AC_CLOSE`** — `mn_cleanup()` (`gemmnlib.c:740-758`), called from
`ap_exit`/`sh_ldapp`'s abandoned-`appl_init` cleanup (§4 below), loops over
every registered slot:
```c
for (i = 0; i < NUM_ACCS; i++)
    if (desk_ppd[i])
        ap_sendmsg(appl_msg, AC_CLOSE, desk_ppd[i], i, 0, 0, 0, 0);
```
Here **`w3 = i`** — the same menu-id index space `mn_getownid` resolves into
(both index `desk_ppd[]`/`D.g_acctitle[]`, both filled by the same
`mn_register` call, `gemmnlib.c:789-816`) — and **`w4 = 0`**. Result:
**`msg[3] =` the accessory's menu id, `msg[4] = 0`.**

So the asymmetry is real and is exactly what it looks like: `AC_OPEN` is
*routed* through the generic menu-click path (`ct_msgup`, shared with
`MN_SELECTED`, whose `w3`/`w4` are *title*/*item* by convention — see
[`menu.md`](./menu.md) "`MN_SELECTED` carries..."), so the accessory's
identity lands in the "item" slot (`msg[4]`). `AC_CLOSE` is sent directly by
`mn_cleanup` with no title concept at all, so the accessory's identity is
put in the first free word (`msg[3]`). This is not a bug and not
documented anywhere as an explicit design note in EmuTOS — it falls out of
`ap_sendmsg`'s single generic signature being reused by two call sites with
different argument mappings. **Confirms**
[`docs/api/_discrepancies.md:1433`](../_discrepancies.md) and
[`docs/api/aes/menu.md:977-979`](./menu.md), both already recording the
`msg[4]` half; this file adds the `AC_CLOSE`/`msg[3]` half with the same
rigor.

An accessory that only checks `msg[0]` and assumes a fixed slot for "my
menu id" regardless of message type will misroute one of the two messages.
`sdk/examples/sysacc.c:284-289` gets this right (see §5).

---

## 4. What an accessory itself must do

No accessory example ships pre-built in `references/emutos/` (the classic
`.ACC` ecosystem is third-party: CONTROL.ACC, EMULATOR.ACC, etc., named but
not included — `geminit.c:82-83`'s `CP_SHELL_INIT` comment is the closest
thing to an in-tree reference, and it only documents the shell-buffer `#a`
convention those accessories relied on, not the `AC_OPEN`/`AC_CLOSE`
protocol). The concrete, in-repo reference for "what an accessory must do"
is **`sdk/examples/sysacc.c`** (ACC06), reviewed in full in §5 — it is
written directly against this extraction and implements every point below.

From the source (`geminit.c`'s `wait_for_accs` doc-comment, `:569-598`, plus
the message-routing traced in §3):

1. **`appl_init`** (AES 10) — obtain an application id. Required before
   `menu_register` (which needs a `pid`, `gemmnlib.c:794`).
2. **`menu_register`** (AES 35, `int_in[0] = pid`, `addr_in[0] = title
   string`) — register the Desk-menu entry; the return value (`0..5`, or
   `-1` on failure) is the accessory's own menu id, which it must retain to
   match against `msg[3]`/`msg[4]` per §3. `menu_register` stores the
   caller's string **pointer**, not a copy (`gemmnlib.c:806`) — the title
   string must stay valid for the accessory's entire lifetime.
3. **Enter a message-wait loop** — `evnt_mesag()` or `evnt_multi()` with
   `MU_MESAG` set. Either satisfies `wait_for_accs(AP_MESAG)`
   (`geminit.c:576` — "either via `evnt_mesag()` or `evnt_multi()`"); the
   accessory does not need to reach the wait *before* being scheduled — step
   4 of §1 tolerates it not having initialised yet (`geminit.c:580-582`).
4. **React to `AC_OPEN`** (40) — check `msg[4]` against its own menu id (§3);
   if it matches, do whatever the accessory does (show a dialog, open a
   window, …), then loop back to the wait.
5. **React to `AC_CLOSE`** (41) — check `msg[3]` against its own menu id
   (§3). The AES sends this whenever *any* application terminates without
   having called `appl_exit()` itself, or when one does call it
   (`ap_exit`/`mn_cleanup`, `gemaplib.c:251-261`, `gemshlib.c:650-657` — see
   [`application.md`](./application.md) `appl_exit (19)`). An accessory does
   **not** need to do anything to make `wait_for_accs(AP_ACCLOSE)` succeed on
   its behalf beyond having received the message: the `AP_ACCLOSE` flag is
   set by the AES's own event dispatcher the moment a `MU_MESAG` wait
   delivers a message with `msg[0] == AC_CLOSE`
   (`gemsuper.c:146-162`/`gemevlib.c`, cited and traced in
   [`event.md`](./event.md) "Behaviour"/§`AC_CLOSE`) — **not** something the
   accessory's own code sets. The accessory's `AC_CLOSE` handler is free to
   do nothing but loop back to the wait — the message exists so an accessory
   with state tied to the *terminating application* (e.g. a clipboard
   watcher) can react; one with none, like `sysacc.c`, has nothing to do.
6. **Never call `appl_exit`/`Pterm`.** An accessory that terminates loses
   its AESPD's identity semantics silently: its slot stays reserved
   (`fpdnm`/`pd_index` have no liveness test, per
   [`application.md`](./application.md) "The descriptor is not freed") but
   nothing ever schedules it again, so `menu_register`'s slot is permanently
   dead for the rest of the session. EmuTOS accessories are, by convention
   and by the absence of any accessory-specific exit path in `geminit.c`,
   infinite loops.

---

## 5. `sdk/examples/sysacc.c` (ACC06) — contract review

`sdk/examples/sysacc.c` is explicitly marked "THIS PROGRAM CANNOT RUN YET —
IT IS SOURCE, NOT A DELIVERABLE," written ahead of the accessory framework
existing, as a target for ACC02-ACC05 to build against. Reviewed against
this extraction:

- **`AC_OPEN`/`AC_CLOSE` contract: correct.** `msg[0] == AC_OPEN && msg[4]
  == menu_id` (`:290`) and `msg[0] == AC_CLOSE && msg[3] == menu_id`
  (`:296`) match §3 exactly, including the asymmetry — the file's own
  header comment (`:284-289`) states the same citations
  (`gemctrl.c:337-346` → `gemwmlib.c:851-862` for `AC_OPEN`;
  `gemmnlib.c:747` for `AC_CLOSE`) this document independently verified.
  This is not a case of the file copying an unverified claim: its citations
  and this document's were derived independently and agree.
- **Lifecycle shape: correct.** `appl_init` → `menu_register` → infinite
  `evnt_mesag` loop, no `appl_exit`/`Pterm` (§4, points 1-3 and 6) — matches
  precisely, including the explicit comment (`:55-56`) that an accessory
  never terminates.
- **`AC_CLOSE` handling: correct, and correctly minimal.** The handler does
  nothing but keep looping (`:296-300`) — consistent with §4 point 5: this
  accessory owns no window or allocation, so there is nothing *to* tear
  down, and it must not set any "I saw the close" flag itself (the AES does
  that for it).
- **Menu-id retention: correct.** `menu_id` is captured from
  `menu_register`'s return (`:264`) and used for both checks — matching the
  requirement in §4 point 2 that the accessory must retain its own id to
  disambiguate messages meant for a *different* registered accessory (a
  real concern once more than one is loaded).
- **One thing to flag, not a defect: the `apid < 0` / `menu_id < 0` failure
  paths both `return 1`** (`:259-260`, `:266-269`) — i.e. the program simply
  exits if `appl_init` or `menu_register` fails. This is **not** a violation
  of "never call `appl_exit`/`Pterm`" (§4 point 6): that rule describes the
  steady-state accessory that *did* register successfully and is now the
  AES's responsibility to message; a program that never got as far as
  registering has nothing for the message protocol to break. EmuTOS itself
  has no special handling for this case either — `load_one_acc` doesn't
  distinguish "loaded and ran to registration" from "loaded and exited
  early" (`geminit.c:236-257`); a slot with no matching `desk_ppd[]` entry
  is simply never a target of `AC_OPEN`/`AC_CLOSE`, so `sysacc.c` exiting
  here is consistent, not an EmuTOS-contract gap.

**Conclusion: `sysacc.c` gets the `AC_OPEN`/`AC_CLOSE` contract right**,
including the asymmetric message-word placement, matching this session's
independent source derivation. No correction needed to the file.

---

## 6. Gating — is the whole lifecycle on for our reference profile?

**Profile:** Falcon030/TOS4, `make 512` → `-DTARGET_512`, `REF_OS=TOS404`
(`references/emutos/Makefile:558-565`).

**`TARGET_192` check (explicitly asked for).** `config.h:210` is the start
of the `#ifdef TARGET_192` block (`"This target is only useful on ST
hardware... strip out all the advanced features"`), running to `:401`
(immediately before `#ifdef TARGET_256` at `:402`). **`TARGET_512` never
defines `TARGET_192`**, and `config.h` has no `#ifdef TARGET_512` block at
all (confirmed by exhaustive grep for every `#ifdef TARGET_*`/`#if
defined(TARGET_*)` line in the file — the only blocks are `192`, `256`,
`CART`, `FLOPPY`, `AMIGA_ROM`, `AMIGA_FLOPPY`, `LISA_FLOPPY`, none of which
`TARGET_512` triggers). So `TARGET_512` falls through to every feature's
**generic, full-featured default**, defined later in the file
(`config.h:1250-1330` "Software Section - AES", `:1744-1750`) — the same
defaults TOS4/Falcon ships with.

None of the accessory *lifecycle* functions themselves
(`count_accs`/`load_one_acc`/`load_accs`/`free_accs`/`run_accs_and_desktop`/
`wait_for_accs`/`mn_register`/`mn_cleanup`/`ap_sendmsg`'s `AC_OPEN`/
`AC_CLOSE` posting) are wrapped in **any** `#if CONF_WITH_*` guard — they are
unconditional core GEM/AES code in every EmuTOS build, `TARGET_192` included.
`NUM_ACCS = 6` itself is defined unconditionally in `sysconf.h:37-40`
(a file whose own header states its definitions are **not** overridable by
`localconf.h`).

Flags *adjacent* to the lifecycle (called from the same `run_accs_and_desktop`
init sequence, or affecting message content) and their TARGET_512 state:

| Flag | Default (generic fallback) | Relevance | On for TARGET_512? |
|---|---|---|---|
| `CONF_WITH_MENU_EXTENSION` | `1` (`config.h:1304-1306`) | `mnext_init()` runs in `run_accs_and_desktop` (`:678`); gates whether `MN_SELECTED`/`AC_OPEN`'s `msg[5..7]` carry the tree pointer (`gemctrl.c:347-349,355-358`) — `AC_OPEN`'s `msg[3]`/`msg[4]` are unaffected either way | **Yes** |
| `CONF_WITH_3D_OBJECTS` | `1` (`config.h:1258-1260`, "as in Atari TOS 4") | `init_3d()` runs in `run_accs_and_desktop` (`:682`) | **Yes** |
| `CONF_WITH_BACKGROUNDS` | `1` (`config.h:1747-1749`) | `set_aes_background()` (`:635-641`), called from `run_accs_and_desktop` right after accessory load, if `#Q` was found in `EMUDESK.INF` | **Yes** |
| `CONF_WITH_LOADABLE_CURSORS` | `1` (`config.h:1290-1292`) | `setup_mouse_cursors()` runs before accessory load (`:664`) — incidental to `run_accs_and_desktop`, not to the accessory concept | **Yes** |
| `CONF_WITH_PCGEM` | `1` (`config.h:1319-1321`) | Not directly touched by the accessory lifecycle; gates `appl_yield` (17), which cooperating accessories may call | **Yes** |

**Plain answer: every part of the `.ACC` lifecycle is on in our reference
build.** There is no `CONF_WITH_*` flag that gates accessory support itself,
and every flag incidentally exercised by the same init sequence defaults on
for `TARGET_512`. The `TARGET_192` trap the ticket warns about does not
apply here — `TARGET_512` has no special-case block to be caught by.

---

## 7. The extension question — what `.NAC` costs

**Lead decision (communicated mid-task, ADR-018 family): the native
accessory extension is `.NAC`.** This section works out what that choice
costs, as directed, rather than recommending an extension.

### 7.1 What `count_accs`'s pattern becomes, and where it lives

EmuTOS's own line is `strcpy(D.g_work,"\\*.ACC");` (`aes/geminit.c:272`) —
this is EmuTOS source, not ours, and is never modified (NewTOS is a
clean-room reimplementation; no EmuTOS C is linked in). **There is currently
no NewTOS equivalent to change**: an exhaustive search of `kernel/src/` and
`tos-api/src/` for `count_accs`/`load_accs`/`NUM_ACCS`/any `.ACC` literal
found only the already-known `menu.rs:1093` `NUM_ACCS` constant (the
registration-slot table, §7.3) — no scan, no loader, nothing that touches a
directory wildcard at all. So ACC02 is **writing** a `count_accs`-equivalent
from scratch, modelled on but not copied from `geminit.c:264-284`, and per
the lead's separate two-place decision (§7.3) it becomes two scans, not one:
`\*.NAC` in the root and `*.NAC` inside `\ACC\`.

### 7.2 Does anything else in the lifecycle key off the literal extension?

No. Tracing every reference to `"ACC"`/`.ACC` in `aes/geminit.c` and the
files reachable from it: the pattern string is built and consumed entirely
within `count_accs()` (`:264-284`) via `dos_sfirst`/`dos_snext`'s FAT-style
8.3 wildcard match (`bdos/fsdir.c:737-796` `builds()`, `:1802-1845`
`match()`). Everything downstream — `load_one_acc`'s `dos_open`/`pgmld`
(`:236-257`), the AESPD numbering (`shell-loop.md` §4), `menu_register`'s
slot table, `AC_OPEN`/`AC_CLOSE` routing (§3) — operates on the **file
name already resolved by `count_accs`**, not on the extension string
itself. So a NewTOS `.NAC` accessory needs no downstream changes beyond the
scan pattern: once a file is named as accessory candidates, `load_one_acc`,
`menu_register`, and the message protocol are extension-agnostic in EmuTOS
and will stay so in NewTOS.

### 7.3 The two-place search — a NewTOS-own rule, not EmuTOS's

**Marked explicitly as ours: EmuTOS has exactly one search location
(`\*.ACC` in the root, `geminit.c:272`) and therefore no precedence question
at all — there is no EmuTOS line to cite for the answer below.**

Per lead direction: NewTOS searches `C:\ACC\*.NAC` **first**, then
`C:\*.NAC` in the root; a file name already loaded from `\ACC\` is **skipped**
if the same name also exists in the root. Dedupe is by **file name**, not by
registered menu title — the title is a runtime fact only available once the
accessory has run past its own `menu_register` call (§4 point 2), far too
late to decide at scan time whether to load a *second* file of the same
name. Given as the lead's stated reasoning.

**What the shared `NUM_ACCS = 6` budget must do across two locations — this
part *does* have a source answer.** `sysconf.h:37-40`'s comment is explicit
that `NUM_ACCS` is simultaneously "the maximum number of desk accessory
files (`.ACC`) that will be loaded **AND** the maximum number of desk
accessory slots available (one slot per `mn_register()` call)" — i.e. EmuTOS
already treats the load-count and the registration-count as one shared pool,
not two independent budgets, even within its single search location (the
fixed-size `static ACC acc[NUM_ACCS]` array at `geminit.c:109` structurally
cannot hold more than 6 names regardless of how many `.ACC` files exist).
**What EmuTOS does when the cap is hit mid-scan**: `count_accs`'s loop
(`for (i = 0; i < NUM_ACCS; i++) { rc = dos_sfirst/snext(); ...; acc[i].name
= ...; }`, `:275-281`) simply stops calling `dos_snext()` once `i` reaches
6 — the loop condition, not an explicit break/error, ends the scan. Any
7th+ matching file in the directory is silently never examined; there is no
warning, no error return (`count_accs` always returns a count `<= NUM_ACCS`
via the loop bound alone). **The two-place scan must inherit this
shared-total, silent-truncation behaviour rather than invent a second
budget**: a combined counter across both `\ACC\` and the root, stopping
(without diagnostic) at 6 total, is what "the same NewTOS's own array size"
(the fixed `[NUM_ACCS]` tables `menu.rs:1093-1103` already declares) requires
structurally, exactly as `geminit.c:109`'s fixed array required it for
EmuTOS's single location. The `\ACC\`-first ordering (lead's decision, this
section) determines *which* 6 win when more than 6 exist in total, not
whether more than 6 can ever be loaded.

The `FA_RO` filter (`geminit.c:277`, passed to `dos_sfirst`) is not itself a
"per-place" concept in EmuTOS's matching code (`fsdir.c:1836-1844`
`match()`) — it is an attribute mask applied per candidate directory entry,
independent of which directory is being scanned. It has no reason to differ
between the two NewTOS locations and should be applied identically to both.

### 7.4 No existing Atari `.ACC` will ever be found — and could not run anyway

Confirmed **from source**, not from the expectation stated in the brief.
`load_one_acc` (`geminit.c:236-257`) loads the file through `pgmld`
(`bdos/kpgmld.c:40` onward, `kpgmhdrld`/`kpgmld`) — the same GEMDOS
program-header + relocation-table loader every classic `.PRG`/`.APP`/
`.TOS`/`.TTP` uses (`pghdr.h`'s header format, magic-number-checked at
`kpgmld.c:43-45`). The loaded bytes are then handed to `pstart(gotopgm,
...)` (`geminit.c:255`), and `gotopgm` (`aes/gemasm.S:73-131`) is **m68k
assembly** whose own comment states its job precisely: "go to the start of
the accessory TEXT segment in user mode... The register a0 must contain the
address of the basepage" (`gemasm.S:76-81`) — i.e. it jumps directly into
the loaded file's own machine code. A classic `.ACC`'s *contents* are
compiled 68000 instructions; nothing about the file-extension convention
changes that. So the question of whether NewTOS's loader could run a
found `.ACC` is moot for a structural reason independent of naming: **the
loader is architecture-bound by the file's contents, not by which extension
found it.** This is unaffected by NewTOS choosing `.NAC` over `.ACC` —
even if NewTOS scanned for literal `\*.ACC` files, it could not execute what
it found without the future m68k emulator. Choosing `.NAC` therefore costs
nothing on this axis: it does not create a new incompatibility, it just
means NewTOS's own native accessories (real, AArch64-ELF, running code) are
findable by a pattern that will never collide with a leftover classic
`CONTROL.ACC` sitting in the same directory — which, if `.NAC`'s scan used
`*.ACC` instead, NewTOS would find, attempt to load as a native ELF, and
reject or crash on, for no benefit (it still couldn't run the m68k content
either way).

### 7.5 Where the extension is classified today, and the ACC02 gap

`kernel/src/desk/ftype.rs`'s `classify()` (`:59-72`, `FileType`/`Flavor`
enums `:20-46`) is the single point ADR-018 names for filename → launch-
behaviour classification, used by `run_auto_folder` and the desktop's
`open_item` launch path per the doc comment at `ftype.rs:1-18`. **As of this
session (`HEAD f1cf69d`), `classify()` recognises exactly four extension
groups: `.NAP`/`.NCP`/`.NTP` (`FileType::Native`) and
`.PRG`/`.APP`/`.TOS`/`.TTP`/`.GTP` (`FileType::M68k`); everything else,
including any `.NAC`/`.ACC` name, falls through to `FileType::Doc` (the
`_ => FileType::Doc` arm, `:72`).** There is no `Flavor::Accessory` variant
and no `.NAC` case in the match arm.

Cross-checked against `docs/MEMORY.md`'s **ADR-018** (`:1004-1061`, dated
2026-07-16): as recorded on disk it documents only the three-member `.NAP`/
`.NCP`/`.NTP` family and the routing table for `open_item`/
`run_auto_folder`; it contains no `.NAC` entry and no note dated 2026-09-03.
Flagging this factually rather than silently assuming the coordinator's
description is already reflected on disk: **the pinned `ftype.rs` and the
pinned `ADR-018` text do not yet contain `.NAC`, regardless of when the name
was chosen.** This is exactly the gap ACC02 exists to close, not a
discrepancy in this extraction.

**What ACC02 needs to add** (note only — no code changed here):
- A `Flavor`/`FileType` case for `.NAC`, matched case-insensitively like the
  other three (`classify`'s existing 4-byte-suffix comparison, `:59-71`,
  extends directly).
- Since an accessory is not launched like a `.NAP`/`.NCP`/`.NTP` program
  (it is *loaded once at AES start*, never double-clicked — §1), `classify`
  returning "this is an accessory" is necessary but not sufficient:
  `run_auto_folder`/`open_item` must **not** attempt to `run_program` a
  `.NAC` file the way they do a `.NAP` — a double-click on a `.NAC` in the
  desktop needs its own decision (EmuTOS has none either: an `.ACC` is never
  double-click-launched, only auto-loaded at boot from a fixed scan,
  §1 step 3), left for ACC02 to specify.

### 7.6 The stray `C:\ACC\` directory on the physical card

**Question:** can a bare `ACC\` directory (no extension) at the SD-card
root confuse EmuTOS's `\*.ACC` root scan? Traced through the exact matching
code rather than assumed:

`builds()` (`bdos/fsdir.c:737-796`) converts the search pattern `"*.ACC"`
into the padded 11-byte FCB form `"????????ACC"` — 8 wildcard bytes for the
name, then the literal `"ACC"` for the extension (confirmed by tracing the
function's character-by-character logic: the leading `*` produces `'?'`
padding for the name field per `:763-776`, and the literal `"ACC"` is
copied verbatim into the extension field per `:782-783`, no wildcard
substitution since it is not followed by another `*`). A directory named
plainly `"ACC"` (no extension) is stored, in the same 11-byte FCB form, as
`"ACC     "` + **three blank spaces** for the extension — not `"ACC"`.
`match()` (`fsdir.c:1802-1845`) compares the extension bytes literally
(`:1830-1833`, only `'?'` positions are skipped) — position 8 of the
pattern is `'A'`, position 8 of the directory's stored name is `' '`;
`toupper('A') != toupper(' ')`, so `match()` returns `FALSE` **on the name
comparison alone**, before the attribute check is even reached.

Independently, even a hypothetical directory whose name *did* match by
characters would still be excluded: `count_accs` searches with
`att = FA_RO` (`0x01`, `bdosdefs.h:36`); a directory entry carries
`FA_SUBDIR` (`0x10`, `bdosdefs.h:40`) in its own attribute byte; `match()`'s
final attribute test (`fsdir.c:1840-1844`) is `(*s1 & *s2) ? TRUE : FALSE`
for any non-`FA_VOL`/non-`FA_SUBDIR` search attribute matching a non-zero-
attribute entry — `0x01 & 0x10 == 0`, so the bitwise AND fails too.

**Answer: no, the stray `ACC\` directory cannot confuse EmuTOS's root scan,
for two independent reasons (name mismatch, and attribute mismatch even if
the name had matched).** `tools/make-sd-image.sh:59` (`mkdir -p "$V/ACC"`)
is generic test-layout filler alongside `GEMSYS`/`SUB` (icon-label-layout
test content, per the script's own header comment `:46-49`) — it has no
functional relationship to accessory loading today, since no NewTOS code
reads it. Once ACC02 lands the lead's two-place scan (§7.3), that same
directory stops being incidental and **becomes** the intended `\ACC\` search
root — not "confused," but repurposed by design.

---

## 8. NewTOS gap list → ACC02-ACC05

Everything below is confirmed absent from `kernel/src/` and `tos-api/src/`
by direct search this session (not inferred from the ticket's framing):

1. **No discovery/load path at all.** No `count_accs`/`load_one_acc`/
   `load_accs`/`free_accs` equivalent exists — `menu_register`'s comment
   (`tos-api/src/aes/menu.rs:1087-1092`) and `pd.rs:530-533`'s note ("
   `mn_cleanup` and `wait_for_accs` concern desk accessories, which NewTOS
   has none of") both already say this plainly in the code. → **ACC02**
   (§1 steps 1-3, §7's two-place scan, §7.5's `ftype.rs` addition).
2. **No slot-reservation/AESPD-numbering integration for accessories.**
   NewTOS's process-identity table (documented in
   [`shell-loop.md`](./shell-loop.md) §4, implemented in `tos-api/src/aes/pd.rs`)
   has no accessory slots wired in the way `geminit.c:812-877` reserves
   slots 2..`num_accs+1`. → **ACC02**.
3. **`AC_OPEN` is never posted.** The Desk-menu drop-down insertion
   (`menu_fixup`/`gl_dafirst`, already correctly described in
   [`menu.md`](./menu.md)) and the click-routing branch
   (`gemctrl.c:337-350`, §3 above) have no NewTOS counterpart — `menu.rs`'s
   own comment says so (`:1089-1091`) and `kernel/src/desk/main.rs:1444-1462`
   documents that the desktop *correctly* does not handle it either, because
   the AES is supposed to. → **ACC03**.
4. **`AC_CLOSE` is never posted.** `mn_cleanup` (§3, §4 point 5) has no
   NewTOS implementation; `appl_exit`'s NewTOS counterpart (`pd.rs:498-543`
   `close()`) explicitly says "`mn_cleanup`... concerns desk accessories,
   which NewTOS has none of" and does not send it. → **ACC03**.
5. **`wait_for_accs` has no accessories to wait for.** Both call sites
   (`ap_exit`'s `AP_ACCLOSE` wait and `run_accs_and_desktop`'s post-load
   `AP_MESAG` wait) are vacuous today by construction (zero accessories
   exist), not implemented-and-tested against a real accessory process.
   → **ACC04** (once ACC02/ACC03 exist, this needs a real multi-process
   regression, not just a no-op-path unit test).
6. **No accessory example can run.** `sdk/examples/sysacc.c` registers
   successfully today and then blocks forever in `evnt_mesag` (its own
   header, `:29-31`, confirmed accurate by this review, §5) — it is the
   integration target for ACC02-ACC04, and its extension is still `.TBD`
   pending `.NAC` wiring (`sysacc.c:33-43` — now resolved by the lead's
   decision, §7). → **ACC05** (build + install it as `SYSACC.NAC` once
   ACC02-ACC04 land, replacing the `.TBD` placeholder build step in
   `sysacc.c`'s own header comment).

---

## Related

- [`menu.md`](./menu.md) — `mn_register`/`menu_fixup`/`MN_SELECTED` vs.
  `AC_OPEN` message-layout details this file cites rather than repeats.
- [`shell-loop.md`](./shell-loop.md) — `gem_main`'s full sequence, AESPD
  numbering (0 = dos-appl, 1 = SCRENMGR, 2+ = accessories), `sh_ldapp`'s
  abandoned-`appl_init` cleanup.
- [`application.md`](./application.md) — `appl_exit`/`appl_init` full
  contract, the "descriptor is not freed" finding this file's §4 point 6
  depends on.
- [`event.md`](./event.md) — `AP_MESAG`/`AP_ACCLOSE` flag-setting inside
  the event dispatcher, cited in §4 point 5.
- [`_state.md`](./_state.md) — the DA-slot global-variable table
  (`D.g_acctitle[]`, `desk_ppd[]`, `acc_display[]`, `gl_dafirst`).
- [`_discrepancies.md`](../_discrepancies.md) — the prior `AC_OPEN`/`msg[4]`
  finding this file's §3 confirms and extends with the `AC_CLOSE`/`msg[3]`
  half.
- `kernel/src/desk/ftype.rs`, `docs/MEMORY.md` ADR-018 — the extension
  classification this file's §7.5 finds has no `.NAC` entry yet.
- `sdk/examples/sysacc.c` — the accessory this file's §5 reviews.
