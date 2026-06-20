# The Cookie Jar (system feature registry)

> **Phase-00b extraction (added for Phase-08 D5).**
> Source: EmuTOS @ `1a7484ab` — `include/cookie.h` (struct + tags),
> `util/cookie.c` (`cookie_init`/`cookie_add`), `bios/machine.c:665`
> (`fill_cookie_jar`), `bios/bios.c:277-280` (boot wiring),
> `cli/cmdutil.c:321` (`getcookie`, app-side access),
> `util/boot.c:72` (`get_cookie`, supervisor-side access).
>
> The cookie jar is **not a trap** — it is a system data structure that
> programs read through a fixed system variable. This document records
> what it is and, crucially, **how applications access it**, because the
> classic access path does not exist on NewTOS (see the NewTOS section
> and ADR-012).

## Structure

```c
struct cookie { ULONG tag; ULONG value; };   /* include/cookie.h:16 */
```

The jar is a contiguous **array** of `{tag, value}` entries. The last
entry is a **terminator** with `tag == 0` whose `value` field holds the
jar's **total slot capacity** (not a value). Readers scan from the start
until they hit `tag == 0`.

EmuTOS allocates a fixed 20-slot jar (`util/cookie.c:24`):

```c
static struct cookie dflt_jar[20];
void cookie_init(void) {            /* util/cookie.c:26, called bios.c:278 */
    dflt_jar[0].tag   = 0;
    dflt_jar[0].value = 20;         /* {0, capacity} terminator */
    p_cookies = dflt_jar;           /* publish via the $5A0 sysvar */
}
```

`cookie_add(tag, value)` (`util/cookie.c:34`) walks to the terminator,
and if there is room (`count < capacity-1`) writes `{tag, value}` into
the terminator slot and pushes a fresh `{0, capacity}` terminator one
slot down. So the jar grows in place; the capacity word is preserved.

## How it is built (`fill_cookie_jar`, `bios/machine.c:665`)

Called once at boot (`bios.c:280`) **after** `cookie_init`. It runs
hardware detection and `cookie_add`s the results. Tags (`cookie.h`):

| Tag (ASCII) | Hex | Meaning | NewTOS relevance |
|-------------|-----|---------|------------------|
| `_CPU` | `0x5F435055` | m68k CPU type (0/10/20/30/40/60) | n/a (AArch64) — re-purpose or omit |
| `_VDO` | `0x5F56444F` | video shifter revision (ST/STe/TT/Falcon) | **meaningless** (no Shifter) |
| `_FPU` | `0x5F465055` | m68k FPU type | **meaningless** |
| `_MCH` | `0x5F4D4348` | machine type | re-purpose: a NewTOS machine id |
| `_SND` | `0x5F534E44` | sound feature bitmap (PSG/DMA/DSP) | **meaningless** (no PSG) |
| `_FDC` | `0x5F464443` | floppy controller type | **meaningless** (no FDC) |
| `_AKP` | `0x5F414B50` | keyboard/language (low word) | meaningful (IKBD layout) |
| `_IDT` | `0x5F494454` | date/time display format | meaningful (NewTOS has a clock) |
| `_FLK` | `0x5F464C4B` | file/record locking present | meaningful (GEMDOS FS) |
| `_5MS` | `0x5F354D53` | 200 Hz timer vector (for MiNT) | n/a |
| `_NF`/`XHDI`/`NVDI`/… | — | extension drivers | out of scope |

Most standard cookies describe **Atari hardware NewTOS does not emulate**
(non-goal: no Shifter/PSG/FDC/m68k-FPU). A faithful NewTOS jar therefore
carries only the still-meaningful tags plus **its own** cookie(s) so
ported programs can detect the NewTOS environment and its real features.

## How applications access it — the critical part

There is **no GEMDOS/BIOS/XBIOS function** for the cookie jar in classic
TOS / EmuTOS. Access is always: **read the system-variable pointer at
`$5A0` (`_p_cookies`), then scan the array.** Reading `$5A0` is a
supervisor-only low-memory access, so two patterns exist:

**1. Application context** — must elevate via `Supexec` (XBIOS 0x26):
```c
/* cli/cmdutil.c:321 */
jar = (COOKIE *)Supexec(getjar);          /* getjar returns *(COOKIE**)0x5A0 */
for (c = jar; c->cookie; c++)
    if (c->cookie == tag) { *pvalue = c->value; return 1; }
return 0;
```

**2. Supervisor context** — direct dereference:
```c
/* util/boot.c:72 */
struct cookie *cptr = _p_cookies;         /* the $5A0 variable */
do { if (cptr->id == id) return cptr->value; } while ((++cptr)->id);
```

So the access contract an application relies on is:
**`Supexec` + read `$5A0` + linear scan until `tag == 0`.**

## NewTOS implementation notes

The classic access path is **blocked twice** on NewTOS:

1. **`$5A0` is not mapped.** NewTOS does not map Atari low-memory system
   variables onto AArch64 addresses (same decision as the `$420`/`$432`
   BIOS vars — direct poke is out of scope, m68k-emulator-only). So
   `*(COOKIE**)0x5A0` cannot work for native AArch64 programs.
2. **`Supexec` is deferred.** The AArch64 substitute for `Supexec` is an
   open decision (Open-ADR *ADR-Supexec*, blocks PHASE-06). Without it,
   even the elevation half of the app pattern is unavailable.

Therefore the cookie jar cannot be implemented by mirroring EmuTOS; it
needs (a) **NewTOS's own cookie set** (the meaningful tags + a NewTOS
identity cookie) and (b) **a NewTOS-native access mechanism** that
replaces "`Supexec` + `$5A0`".

**Decided (ADR-012, Accepted 2026-06-20):** the jar is **kernel-owned** —
allocated from the kernel heap, pointer held in a kernel-internal global
(no `$5A0`, no fixed address). Access is by **function returning content,
not a pointer**: `Getcookie(tag) -> (found, value)` (the kernel scans its
own jar; the jar is EL1-only memory so a pointer would be useless to EL0
anyway), with `Setcookie(tag, value)` to add/update. No user mapping, no
`Supexec`. This spec is the access-agnostic data model; the function ABI
+ the NewTOS cookie set are pinned in the implementation spec.

## Cross-references

- ADR-012 (cookie-jar access mechanism) — `docs/MEMORY.md`
- Open-ADR *ADR-Supexec*, `$420`-vars decision — STATUS Recent-Decisions 2026-05-03
- Compendium: "Cookie Jar"; Profibuch: "Cookie-Jar" — both document the
  `$5A0` + scan contract; cross-check pending.
