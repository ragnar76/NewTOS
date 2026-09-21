# A48 — Design: `shel_write(SHW_RESCHNG)` on NewTOS (a fixed-resolution board)

Date: 2026-09-21 · Role: Researcher · Ticket: [`TASK/A48.md`](../../TASK/A48.md)
(A-5 in [`docs/api/aes/_full-function-inventory.md`](../api/aes/_full-function-inventory.md)
Teil II) · EmuTOS pinned at the tree under `references/emutos/` (no local
modifications found by inspection; commit not independently re-verified this
session — see Open questions).

This is a **design note**, not an implementation and not a spec edit beyond
two small precision fixes noted in §7. Every EmuTOS claim below cites
`file:line`; NewTOS claims cite the current source read this session.
Per the ticket rules: documentation/design only, no `.rs` edits.

---

## 0. One-sentence summary

EmuTOS's `sh_write(SHW_RESCHNG, …)` unconditionally records a pending
restart and **never validates the requested mode itself** (that filtering
lives in the *caller*, not in `sh_write`); the restart it triggers closes
the whole AES session (every running program, not just a video mode) and is
therefore AES protocol that NewTOS should reproduce — but NewTOS's boot
sequence today has **no mechanism to re-enter the AES at all** (`sh_main` is
called exactly once and its return **halts the machine**), so wiring the
missing state into the existing loop condition without first building that
re-entry path would turn today's harmless no-op into a hang/halt on the very
call this ticket is about fixing. That gap — not the video-mode switch
itself — is the real dependency A48.md's own "Einordnung" flags.

---

## 1. Correcting a premise: `sh_write` does not call `check_moderez`

The ticket's brief suggested modelling NewTOS's behaviour on
"`check_moderez`" returning "0 = no change required" inside `shel_write`.
That is not how EmuTOS is built. **There are two independent producers of
`gl_changerez`/`gl_nextrez`, and only one of them validates anything:**

| Producer | Source | Validates against current mode? |
|---|---|---|
| `process_inf1()` — boot-time `.INF` `#E`-line parse | `aes/geminit.c:343-392`, calls `check_moderez(mode)` at `:369` | **Yes** — `check_moderez` (`bios/screen.c:407-423`) returns `0` if `!rez_changeable()` (`:409-410`) or if the requested value doesn't differ from the current one (per its own doc comment, `:392-401`: "we only indicate a change if … the specified value differs from the current value") |
| `sh_write(SHW_RESCHNG, isgem, isover, …)` — the runtime AES call | `aes/gemshlib.c:157-196`, the `case SHW_RESCHNG` arm at `:187-191` | **No.** `gl_changerez = 1 + isover; gl_nextrez = isgem;` unconditionally. No call to `check_moderez`, no comparison to the current mode, no rejection path of any kind. |

The "avoid a no-op restart" filtering that *does* exist for the interactive
desktop path lives in the **caller**, not in `sh_write`: EmuTOS's own
resolution dialog explicitly checks `if (i == selected) /* no change */
return 0;` for both the Falcon and TT cases (`desk/deskrez.c:213-215`
`change_falcon_rez`, and the analogous `i == selected` check in
`change_tt_rez`, `:118-126`) **before** it ever calls `shel_write`. A
caller that bypasses this UI-level check (any third-party program, e.g.
TeraDesk — see §3) and calls `shel_write(SHW_RESCHNG, …)` directly gets the
unconditional behaviour: `sh_write` restarts the AES regardless of whether
the requested mode equals the current one.

**Consequence for the design:** "the AES validates the requested mode like
`check_moderez`" is not a faithful model for `shel_write` itself. The
mode-equality check, if NewTOS wants one, belongs in NewTOS's *own* future
resolution-change UI (mirroring `deskrez.c`), not in the `shel_write` opcode
handler. The opcode handler's job is to record the request unconditionally,
exactly as EmuTOS's is.

---

## 2. The EmuTOS sequence, cited end to end

### 2.1 `sh_write`'s `SHW_RESCHNG` arm

```c
case SHW_RESCHNG:   /* change resolution */
    gl_changerez = 1 + isover;
    gl_nextrez = isgem;
    D.s_cmd[0] = '\0';
    ad_stail[0] = '\0';
    break;
```
(`aes/gemshlib.c:187-191`.) Returns `TRUE` unconditionally like every other
mode (`:195`). `isgem` carries the family-dependent `+2` ST/TT offset
already documented as D-101 in
[`scrap-shell.md`](../api/aes/scrap-shell.md#shel_write-121--sh_write) —
unaffected by this ticket, cited here only because it is the same call.

### 2.2 The `sh_main` loop notices, but only when the *current* program exits

```c
do {
    sh_chdef(psh); sh_chgrf(psh);
    if (gl_shgem) { wm_init(); ratinit(); sh_draw(D.s_cmd, TRUE); }
    if (rc && !IS_BIOS_ERROR(rc)) fm_error(-rc-31);
    rc = sh_ldapp(psh);
} while ((psh->sh_doexec != SHW_SHUTDOWN) && !gl_changerez);
if (gl_shgem) sh_toalpha();
```
(`aes/gemshlib.c:692-731`.) `shel_write` does **not** interrupt the running
program — it returns `TRUE` immediately and the caller keeps running.
`gl_changerez` only takes effect the next time the loop re-checks its
condition, i.e. **after `sh_ldapp` returns**, which happens when the
currently running program terminates (`Pterm`/return from `main`). When it
does, the loop exits *without* launching whatever `sh_nextapp` would
otherwise have selected (normally the desktop, `gemshlib.c:639-643`) —
that is the first half of "closes everything": the *next* scheduled program
never runs.

### 2.3 `gem_main` returning triggers a full AES restart, not a mode switch by itself

```c
void gem_main(void) {
    …
    if (!gl_changerez) process_inf1();   /* :775 */
    if (gl_changerez) {
        switch(gl_changerez) {
        case 1: Setscreen(-1L, -1L, gl_nextrez-2, 0); … break;  /* :782-784 */
        case 2: Setscreen(0L, 0L, FALCON_REZ, gl_nextrez); break; /* :788 */
        }
        gsx_wsclear();
        dos_sdrv(bootdev);
    }
    ml_ocnt = 0;
    gl_changerez = FALSE;                /* :806 */
    …                                     /* mn_init, count_accs, process table
                                             init, iprocess(SCRENMGR), … — every
                                             AES global gets reinitialised */
    aes_run_rom_program(accdesk_start);  /* :893 — runs accessories + desktop
                                             from scratch */
    …
}
```
(`aes/geminit.c:753-899`, elided for length; the elided section is the
*entire* process-table/event-system/screen-manager bring-up that also runs
at cold boot — nothing about it is resolution-specific.) The assembly
wrapper is what makes this a *restart*, not a one-shot:

```asm
aes_restart:
        …                       ; re-zero the whole 'theglo' AES global area
        jsr     _gem_main
        move.w  _gl_changerez,d0
        tst.w   d0
        jne     aes_restart     ; gl_changerez still set -> loop back
```
(`aes/gemstart.S:86-124`.) So: `gem_main()` returning with `gl_changerez`
still nonzero (it is cleared at `geminit.c:806`, *inside* `gem_main`, before
the Setscreen dispatch even runs — wait, re-checking: the clear at `:806`
happens *after* the `if (gl_changerez)` Setscreen block at `:778-793`, so by
the time `gem_main` returns, `gl_changerez` is already `FALSE` again).
**Correction while writing this note:** re-reading `gemstart.S:122-124`,
the assembly checks `gl_changerez` *after* `gem_main` returns — but
`gem_main` itself clears it at `:806`, early in its own body, long before it
returns. That means the `jne aes_restart` branch in `gemstart.S` is only
taken if a **new** `shel_write(SHW_RESCHNG)` request was queued **during**
this `gem_main()` invocation (i.e. by a program that ran inside the fresh
`sh_main` this same `gem_main()` call just started and that is itself
requesting *another* resolution change before terminating this session) —
not by the request that caused the *current* restart. In other words: one
`shel_write(SHW_RESCHNG)` + terminate produces **exactly one** restart, and
`aes_restart` exists to allow a *chain* of resolution changes (change again
from within the freshly restarted session) without returning to
`ui_start`'s outer `Pterm0`. **This refines, not overturns, the design
below** — the restart still happens exactly once per request; only the
"why does the loop exist" detail changes. Flagged here rather than silently
smoothed over, per the "cite what you read, not what seems tidy" rule.

Either way, the observable, load-bearing fact for the design is:
**`gem_main` reinitialises every AES-global structure — the process table,
the event system, the screen-manager process, menu state, accessory list —
and then reruns `accdesk_start` (accessories + desktop) from scratch.**
Nothing about a running foreground GEM application survives across this
boundary; it is a warm restart of the whole AES session, scoped by
`ui_start`'s outer loop so it never falls back to `Pterm0` while a chain of
resolution changes is still pending.

### 2.4 The caller-side "no-op filter" (desktop UI) confirms §1

`change_falcon_rez` (`desk/deskrez.c:152-230`): builds the dialog, and after
`OK`, `if (i == selected) /* no change */ return 0;` (`:213-215`) — the
desktop's `RESITEM` handler (`desk/deskmain.c:897-914`) only calls
`shel_write(SHW_RESCHNG, …)` when `change_resolution()` returned `1`, i.e.
never for "the user re-confirmed the current mode". This is UI-level
courtesy, not an ABI guarantee — see §1.

---

## 3. The caller side: does a real client hang on NewTOS today?

Traced `sdk/apps/teradesk/video.c` and `sdk/apps/teradesk/main.c` in full
(both are in this repo, under `_TOS_ONLY`/`__NEWTOS__` per
`sdk/apps/build-teradesk.sh:87,94`).

- **The call is reachable on NewTOS.** `get_set_video(2)` only calls
  `shel_write` `if (aes_version >= 0x340)` (`video.c:376-379`). NewTOS
  reports `AES_VERSION = 0x0340` (`tos-api/src/aes/appl.rs:110,298`,
  `docs/api/aes/application.md:283-289`), so the gate passes.
- **`vdohi`'s cookie match fails on NewTOS, and that's fine.**
  `find_cookie(C__VDO, &vdo)` (`video.c:158`) never finds a match, because
  NewTOS deliberately omits the `_VDO` cookie
  (`docs/api/bios/_cookie-jar.md:140`, "Deliberately absent (no matching Pi
  hardware)"). `voptions()`'s `switch(vdohi)` therefore falls to
  `default:` — "For the time being, just enable all" (`video.c:539-541`) —
  so **the dialog is fully reachable and shows every ST/TT resolution radio
  button enabled**, with the current one pre-selected from `Getrez()`
  (`video.c:150`, NewTOS `getrez()` returns the fixed code `TT_MEDIUM`
  (`4`), `tos-api/src/xbios/screen.rs:156-166`). Since `fal_mil` stays
  `FALSE` (no Falcon-family cookie recognised either), a change request
  takes the ST/TT branch: `wisgr = currez + 2; wiscr = 0;`
  (`video.c:363-365`) — the `isover = 0` / `+2`-offset arm of D-101.
- **TeraDesk does not hang — it has its own timeout, independent of what
  the AES does.** After `shel_write(SHW_RESCHNG, …)`,
  `if (wait_to_quit()) return;` (`video.c:383`). `wait_to_quit()`
  (`main.c:1664-1700+`) waits at most **2000 ms** (`ev_mtlocount = 2000`,
  `:1667`) for either `AP_TERM` or a TeraDesk-private `RESCH_COMPLETED`
  message; on timeout it returns `FALSE` and `get_set_video` simply falls
  through to its end (the `#if 0`-disabled "attempt to change resolution
  directly" block, `:391-397`, never runs). Control then returns to
  `main.c`'s own shutdown sequence, which — for the resolution-change case
  (`chrez == TRUE`) — explicitly **skips** the hard-reset fallback
  (`if (!shutting && !chrez)`, `main.c:2062`) and falls straight through to
  `appl_exit(); return 0;` (`main.c:2124-2126`), i.e. **TeraDesk terminates
  itself unconditionally within ~2.5 s**, whether or not the AES ever
  restarts.
- **`AP_TERM` is not an EmuTOS message at all.** `grep -rn "AP_TERM"
  references/emutos/` returns zero hits. It is a MiNT-family AES extension
  ("AFAIK only MyAES does that", TeraDesk's own comment,
  `main.c:2010-2013`). EmuTOS never sends it, on shutdown or on resolution
  change, for any caller. **NewTOS does not need to implement it either** —
  TeraDesk's fallback exists precisely because most AESes (EmuTOS included)
  never send it.

**Conclusion for the risk assessment:** the ticket's "does not hang" worry
is already satisfied by TeraDesk's own defensive design, independent of
what NewTOS's `shel_write(SHW_RESCHNG)` does or does not do. The actual gap
(A48/A-5) is a **protocol-fidelity** gap (NewTOS's AES session does not
restart the way a real TOS/EmuTOS AES would), not a **liveness** gap for
this particular caller. A different, less defensive caller that spins
waiting for a visible resolution change (e.g. polling `Getrez()` in a loop)
is conceivable but not evidenced in this codebase; flagged as an open
question in §8.

---

## 4. NewTOS today — the exact current behaviour, and the trap in "just add the flag back"

### 4.1 State recording: confirmed narrower than EmuTOS

`tos_api::aes::shel_write`'s `SHW_RESCHNG` arm
(`tos-api/src/aes/shell.rs:483-488`):
```rust
SHW_RESCHNG => {
    // `D.s_cmd[0] = '\0'; ad_stail[0] = '\0';`
    // (gemshlib.c:190-191). No resolution change on RPi4.
    (*S.s_cmd.get())[0] = 0;
    (*S.s_tail.get())[0] = 0;
}
```
This reproduces only the buffer-clear half of `gemshlib.c:187-191`. It does
**not** record `gl_changerez`/`gl_nextrez` at all — there is no such state
in `tos_api::aes::shell` today (confirmed: no `gl_changerez`/`gl_nextrez`
field in the module, `tos-api/src/aes/shell.rs` grep negative). The A48.md
finding ("EmuTOS setzt `gl_changerez`/`gl_nextrez` … NewTOS leert nur
`s_cmd`/`s_tail`") is accurate as read this session.

### 4.2 The loop's exit condition is missing the term — and the reason given for that is only half right

`kernel/src/shell_loop.rs:69-71`:
> `gl_changerez` is permanently FALSE on RPi4/AArch64 (no `Setscreen`
> resolution family to change to — D-101), so the loop's exit condition
> reduces to `sh_doexec != SHW_SHUTDOWN`.

This is correct for `process_inf1`'s producer (§1: `check_moderez` legitimately
always returns `0` when `rez_changeable()`-equivalent is false — NewTOS has
exactly one fixed mode, negotiated once at boot,
`kernel/src/drivers/mod.rs:17-33`, ADR-017 rev 2 — nothing to change *to*).
It is **not correct** for `sh_write`'s producer: that one is pure AES
software state, set unconditionally by opcode 121/mode 5 regardless of
whether real hardware can honour it (§1, §2.1). The current code and its
comment conflate the two producers; only one of them is legitimately
hardware-gated.

### 4.3 The trap: `sh_main` has no re-entry path, so a literal fix would regress today's no-op into a machine halt

`kernel/src/lib.rs:401-412`:
```rust
let (isauto, isgem) = tos_api::aes::init::process_inf2();
shell_loop::sh_main(isauto, isgem);
// `sh_main` returns only on `shel_write(SHW_SHUTDOWN, …)`.
kprintln!("[shell] shutdown — halting");
arch::aarch64::halt();
```
`sh_main` is called **exactly once**, from the kernel's boot function, and
its comment records the (until now, correct) assumption that it can only
return via `SHW_SHUTDOWN`. If `!gl_changerez()`-equivalent were simply added
back into `shell_loop.rs`'s loop condition (mirroring `gemshlib.c:730`
literally) **without also building something equivalent to `gemstart.S`'s
`aes_restart` wrapper**, the very first `shel_write(SHW_RESCHNG, …)` +
program-terminate on NewTOS would make `sh_main` return, and `lib.rs:412`
would **halt the machine** — worse than today's honest no-op (today:
nothing happens, the desktop comes back exactly as if nothing had been
requested).

**This is the concrete shape of A48.md's own "Einordnung": "Haengt am
Aufloesungswechsel insgesamt, den es bei uns nicht gibt."** It's not the
video-mode switch that's missing (that's legitimately a HAL no-op, §6) —
it's the **AES-session restart loop itself**, which the current boot
architecture has zero support for. `gem_main`'s reinitialisation
(process table, event system, screen-manager process, accessory list,
menu state, `accdesk_start` relaunch — `aes/geminit.c:753-899`) has no
NewTOS counterpart to re-invoke; `kernel::lib::kmain` runs that bring-up
once, inline, as part of one linear boot sequence, not as a re-enterable
function.

---

## 5. Recommended design — two tiers, only one is safe to build now

### Tier 1 — record the intent faithfully, keep it inert (small, low-risk, matches EmuTOS's *recording* half exactly)

Add `gl_changerez`/`gl_nextrez`-equivalent state to `tos_api::aes::shell`
(same module, same pattern as the existing `sh_doexec`/`sh_nextapp`/
`sh_isgem`/`sh_isover` cells — the last of which, `sh_isover`, is exactly
this pattern already shipped for A61: state that is read and retained but
does not yet drive behaviour, with a test-only accessor). `shel_write`'s
`SHW_RESCHNG` arm sets it **unconditionally**, exactly mirroring
`gemshlib.c:187-191` — `gl_changerez = 1 + isover`, `gl_nextrez = isgem`,
no validation, no `check_moderez`-equivalent call (§1). This is a pure,
narrow, EmuTOS-faithful correction of the *recording* gap A48.md
identified, with **no change to the loop's exit condition and no change to
`kmain`'s halt-on-return**, so it cannot regress liveness. It gives the
Developer and future tests a place to observe "did the caller correctly
request a restart" (useful for TeraDesk-style host/QEMU tests, §7) without
touching the part that's actually blocked.

### Tier 2 — the AES-session restart wrapper (separate, larger ticket; blocked on a lead decision, not on missing information)

Build a re-enterable AES session boundary around `sh_main` — the NewTOS
equivalent of `gemstart.S:86-124`'s `aes_restart` loop:

```
loop {
    let (isauto, isgem) = process_inf2();   // already exists, geminit.c:734
    shell_loop::sh_main(isauto, isgem);
    if shell::sh_doexec() == SHW_SHUTDOWN { break; }
    if !shell::gl_changerez() { break; }    // Tier 1's new flag
    shell::clear_gl_changerez();            // `gl_changerez = FALSE` (geminit.c:806)
    // re-seed whatever `gem_main`'s from-scratch bring-up re-initialises
    // that NewTOS actually has — see below
}
kprintln!("[shell] shutdown — halting");
arch::aarch64::halt();
```

What "re-seed" needs to cover is **narrower on NewTOS than on EmuTOS**,
because NewTOS is single-tasking with no accessory (ACC) subsystem today
(confirmed: `kernel/src/shell_loop.rs`'s `sh_ldapp` runs exactly one
foreground program at a time via `run_program_with_tail_rc`, which blocks
until it exits; no concurrent-process/ACC bring-up exists to tear down).
EmuTOS's "closes all applications" therefore collapses, on NewTOS as it
stands, to: reset the AES-global window/menu/shell state
(`shell::sh_main_init`, `window::wm_init`, whatever else the current single
`kmain` bring-up does once) and re-launch the desktop — which is *already*
what happens after any ordinary program exit without a pending resolution
change (`shell_loop.rs:297-303`, the `NORMAL_APP` default-to-desktop path).
The delta Tier 2 actually adds, given no concurrent ACCs exist yet, is
narrow: **exit the loop instead of re-running the *next scheduled* program**
(mirroring §2.2's first half) and **loop back into a fresh `sh_main` instead
of halting** (mirroring §2.3's restart, scoped to what NewTOS has). If/when
NewTOS grows accessories, Tier 2's re-seed step is exactly where their
teardown belongs, matching `free_accs`/`aes_run_rom_program`
re-entry (`aes/geminit.c:762,893`) — noted for whoever builds that later,
not attempted here.

### 6. The Track-B boundary — stays a HAL no-op, unaffected by Tier 1 or 2

The `Setscreen`/`Vsetmode` calls at `geminit.c:782,788` are the *only*
genuinely hardware-gated part of this whole flow. NewTOS's framebuffer is
negotiated once at boot and never re-negotiated at runtime (ADR-017 rev 2,
`kernel/src/drivers/mod.rs:17-33`). Whatever NewTOS resolution-restart
bring-up Tier 2 eventually calls at the equivalent point must **not** call
any XBIOS `Setscreen`/`Vsetmode` implementation expecting it to change
geometry — it stays exactly what it already is (an honest fixed-mode
report, `tos-api/src/xbios/screen.rs:156-166` `getrez()`, already documented).
This matches D-101's existing decision (`docs/api/_discrepancies.md:4230-4234`):
NewTOS records `isgem`/`isover` faithfully but is not required to honour
them as a real mode switch.

---

## 7. Spec corrections made this session

Two small precision edits to existing spec text, both cited above:

1. [`docs/api/aes/scrap-shell.md`](../api/aes/scrap-shell.md) — the
   "NewTOS implementation notes" bullet for `SHW_RESCHNG` previously framed
   the whole feature as "out of scope (no ST/TT-vs-Falcon dual video
   hardware …)". That undersells the reachable gap: the *protocol* half
   (session restart) is reachable and real-caller-relevant (§3) even though
   the *hardware* half (an actual mode switch) is correctly out of scope.
   Rewritten to point at this design note and state the corrected,
   two-tier target precisely (see the file for the exact wording).
2. [`docs/api/aes/shell-loop.md`](../api/aes/shell-loop.md) §7 — added a
   paragraph after the `sh_main` code block documenting the current NewTOS
   deviation precisely (today's loop condition omits `!gl_changerez`
   entirely, for a reason that only half applies — §4.2 above) and
   pointing at this design note, instead of leaving the `[CONTRACT]`
   marking on the loop's exit condition unqualified.

No other spec files touched. `docs/COMPATIBILITY.md` is deliberately **not**
touched this session: per its own convention (see the A57 precedent,
`docs/COMPATIBILITY.md` "Sanctioned deviations" section), an entry there
documents a *shipped* deviation, dated to when it landed — this ticket
ships no code. A COMPATIBILITY.md entry belongs with whichever tier's
implementation lands first.

---

## 8. Decision points for the lead

1. **Tier 1 alone, now, or wait for both tiers together?** Tier 1 is safe
   in isolation (§5) and closes the letter of A48.md's finding (state is
   recorded faithfully) but leaves the *effect* (session restart) still
   missing — a caller inspecting AES state directly (a test, not a real
   TOS program) would see the correct `gl_changerez`/`gl_nextrez`, but a
   real program still would not observe a restart. Recommendation: land
   Tier 1 now (it's small, mirrors A61's `sh_isover` pattern exactly, and
   gives the Developer a foundation), open Tier 2 as its own ticket per
   A48.md's explicit "kein Sammelrefactor" instruction.
2. **Tier 2's scope**: does the lead want the restart wrapper built now
   (given no real caller has been shown to *require* the actual restart —
   TeraDesk survives without it, §3) or deferred until a caller is found
   that depends on it? The GAPS-INDEX framing treats A-5 as a real gap
   regardless of caller impact (protocol fidelity, not just liveness) —
   consistent with "match it, don't rationalize" (`docs/MEMORY.md`,
   *Don't rationalize EmuTOS deviations*) — but Tier 2 is real, non-trivial
   work (a re-enterable boot sequence) that touches `kernel/src/lib.rs`'s
   top-level control flow, which is a bigger surface than a typical A-ticket.
3. **§2.3's `aes_restart`-chaining nuance** (repeated resolution-change
   requests within one restarted session) is real in EmuTOS but has no
   NewTOS-observable consequence found in this session (no evidence any
   real caller chains `shel_write(SHW_RESCHNG)` calls without an
   intervening `Pterm`) — flagged, not designed for, pending evidence.
4. Not a blocker, but noted: this session did not re-pin/verify the
   EmuTOS commit hash independently (the header says "not independently
   re-verified"); prior passes in `scrap-shell.md`/`shell-loop.md` pin
   `1a7484ab` — assumed unchanged, not re-diffed.

---

## 9. Test list for the Developer

**Tier 1 (host, no HW/QEMU needed):**
- `shel_write(SHW_RESCHNG, isgem=X, isover=0)` → the new state reads back
  `gl_changerez-equivalent == 1`, `gl_nextrez-equivalent == X`, `D.s_cmd`/
  `ad_stail` cleared, return value `1`. Mirror for `isover=1` →
  `== 2`. Both cases: unconditional, regardless of what `X` is (no
  validation — assert this explicitly, since it's the corrected premise of
  §1).
- Regression: existing `SHW_EXEC`/`SHW_NOEXEC`/`SHW_SHUTDOWN` host tests
  (`tos-api/tests/aes_shel_write_iscr.rs` and siblings) stay green — Tier 1
  touches only the `SHW_RESCHNG` arm.
- `sh_doexec()`/loop-condition tests: confirm **no behavioural change** —
  `kernel/src/shell_loop.rs`'s loop still exits only on `SHW_SHUTDOWN`
  (Tier 1 deliberately does not wire the new flag in).

**Tier 2 (host + QEMU, once built):**
- Host: seed `gl_changerez`-equivalent, run the (new) outer loop function
  directly (not through `kmain`, to stay host-testable) — assert it loops
  back into a fresh `sh_main` instead of returning, and that the flag is
  cleared before the next iteration (mirrors `geminit.c:806`).
- Host: seed `SHW_SHUTDOWN` instead — assert the loop still breaks (no
  regression of the one exit path that already works).
- QEMU (`raspi4b`, per project convention, `#Z`-autorun or a scripted
  input injection — Memory: *QEMU-Dumps via `#Z`-Autorun*): boot to the
  desktop, run TeraDesk (`_TOS_ONLY`/`__NEWTOS__` build,
  `sdk/apps/build-teradesk.sh`), open the Video options dialog, pick a
  *different* ST/TT resolution than the pre-selected one (confirmed
  reachable per §3 — `default:` case enables all buttons), click OK.
  Expected observable sequence on NewTOS after Tier 2: TeraDesk terminates
  itself (as it does today, §3 — this does not change), the AES session
  restarts (desktop reappears freshly initialised), the physical screen
  geometry is unchanged (1920×1080 in, 1920×1080 out — the Track-B
  no-op, §6). This is the test that actually exercises the restart a real
  caller triggers; nothing in §3's trace requires the video mode to visibly
  change for the protocol-level restart to be verified.
- Regression: run an existing QEMU desktop-interactive scenario
  (Memory: *Desktop interactive verification*) once, unrelated to A48,
  after Tier 2 lands, to confirm the new re-entrant `kmain` loop doesn't
  destabilise ordinary boot (single pass through the loop, `SHW_SHUTDOWN`
  path untouched).

**SHW_SHUTDOWN cross-check (ticket item 4, no design change needed):**
- Confirmed **not** a defect. `can_shutdown()` returns `FALSE` for every
  machine profile except `MACHINE_FIREBEE`/`MACHINE_AMIGA`/`MACHINE_LISA`
  or a detected NatFeats shutdown call (`references/emutos/bios/bios.c:754-770`)
  — the project's Falcon030/TOS4 reference profile
  (`docs/MEMORY.md`, *Target profile: Falcon030/TOS4*) falls into the
  `#else return FALSE` arm (`:768`). NewTOS's `shel_write`
  (`tos-api/src/aes/shell.rs:433-440`) already documents and matches this
  exactly: the request is recorded nowhere, `sh_doexec` keeps its old
  value, the call still returns `1`. No test list item beyond the existing
  coverage; A48.md's own text ("kein Defekt") is confirmed correct.

---

## Files read this session (for traceability, not exhaustive)

`references/emutos/aes/gemshlib.c`, `aes/geminit.c`, `aes/gemstart.S`,
`aes/gemsuper.c`, `desk/deskmain.c`, `desk/deskrez.c`, `bios/screen.c`,
`bios/bios.c`; `sdk/apps/teradesk/video.c`, `main.c`,
`sdk/apps/build-teradesk.sh`; `tos-api/src/aes/shell.rs`,
`tos-api/src/aes/appl.rs`, `tos-api/src/xbios/screen.rs`,
`tos-api/src/cookie.rs`; `kernel/src/shell_loop.rs`, `kernel/src/lib.rs`,
`kernel/src/drivers/mod.rs`, `kernel/src/arch/aarch64/mod.rs`;
`docs/api/aes/scrap-shell.md`, `docs/api/aes/shell-loop.md`,
`docs/api/aes/_full-function-inventory.md`, `docs/api/_discrepancies.md`
(D-101), `docs/api/bios/_cookie-jar.md`, `docs/api/aes/application.md`,
`docs/COMPATIBILITY.md`, `TASK/A61.md`/`A61-STATUS-DONE.MD`.
