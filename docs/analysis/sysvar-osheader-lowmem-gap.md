# Analyse: Low-Memory-Systemvariablen + OSHEADER fehlen für native EL0-Apps

> Erstellt 2026-07-23 als Audit-Vorlage. Auslöser: TeraDesk (native LP64
> GEM-App) freezt beim Start mit weißem Schirm. Dieses Dokument beschreibt
> **worum es geht** und **was zu prüfen ist** — es trifft keine Design-
> Entscheidung, die gehört ins Audit.

## TL;DR

NewTOS legt die klassischen TOS-**Systemvariablen im Low-Memory** (`$400`–`$5FF`)
und den **OSHEADER**, auf den `_sysbase` (`$4F2`) zeigt, **nicht** an. Das wurde
bewusst auf „später, nur für den m68k-Emulator" verschoben. Die Annahme dahinter
— *native recompilierte Apps lesen Systemvariablen über die API, nicht über
Low-Memory* — trägt aber **nicht**: echte geportete TOS-Apps (ADR-019: „1:1
kompilieren, nicht reimplementieren") fassen diese Adressen **direkt** an. Genau
das crasht TeraDesk beim ersten OS-Versions-Check. Der Fix ist kernel-seitig
(Systemvariablen + OSHEADER auch für native Apps bereitstellen), nicht app-seitig.

## Symptom & Fault-Evidenz

TeraDesk auto-gelauncht (QEMU raspi4b, headless), Data Abort **in EL0** (App-Code):

```
[EXC] Data Abort (lower EL): ESR=0x92000004  ELR=0x00011814  SPSR=0x60000000
[EXC] FAR=0x03c0f84507fea943            ← Müll-Adresse (deref eines Müll-Zeigers)
```

Faultende Instruktion in `DESKTOP.NAP` bei `0x11814`:

```asm
mov  w8, #0x4f2      ; _sysbase-Adresse
ldr  x8, [x8]        ; x8 = *(0x4F2)          ← faultet NICHT (0x4F2 in App-VA lesbar)
ldrh w9, [x8, #0x2]  ; os_version = OSHEADER[+2]  ← FAULT (x8 = Müll)
```

Das ist **kein m68k-Assembler**, sondern ganz normaler C-Code, den clang sauber
nach AArch64 übersetzt hat. Quelle: TeraDesk `main.c:56`
`#define os_start (*(long*)0x4F2L)` → OS-Versions-Detection früh in `main()`.

Kern des Befunds: `0x4F2` ist in der App-VA **lesbar** (die App liegt ab Entry
`0x100`), enthält aber **keinen gültigen OSHEADER-Zeiger** — es fehlt nicht die
Page, sondern ein reservierter Sysvar-Bereich + ein befüllter OSHEADER.

## Root-Cause: bewusst deferrte Implementierung

Die Spec existiert vollständig, die Implementierung fehlt — mit expliziter
Begründung an drei Stellen:

- **`docs/api/structures/osheader.md`** (Phase-00 D6, alle Felder spezifiziert),
  „NewTOS implementation notes": *„Empty during Phase 00. PHASE-11/12 will
  populate the m68k-visible OSHEADER once the user emulator can run programs that
  read `_sysbase`."*
- **ADR-012** (`docs/MEMORY.md`, Cookie-Jar): *„`$5A0` is not mapped — Atari
  low-memory sysvars are not given AArch64 addresses … direct poke is out of
  scope, **m68k-emulator-only**."*
- **`docs/COMPATIBILITY.md:84-91`**: *„Native AArch64 source ports read them
  through the header accessors / BIOS calls … not via a raw `*(LONG*)0x4BA`.
  m68k binaries (Phase 10 emulator) get the variables at their real addresses."*

Grep-Befund: in `kernel/src` + `tos-api/src` **null** `_sysbase`/OSHEADER-Instanz,
keine Low-Memory-Sysvar-Befüllung. (Der Cookie-Jar existiert kernel-owned, per
Funktions-Query — **nicht** an `$5A0`, ADR-012.)

## Der eigentliche Konflikt (das Kernthema fürs Audit)

Die gesamte Systemvariablen-Architektur beruht auf einer **Zwei-Welten-Annahme**:

| Welt | wie Systemvariablen gelesen werden (angenommen) |
|------|--------------------------------------------------|
| native recompilierte App | über die **API** (BIOS/XBIOS-Traps, `Ssystem`, Accessoren) |
| m68k-Binary (Emulator, später) | direkt über **Low-Memory** (Emulator mappt `$000`–`$800`) |

**Diese Annahme kollidiert mit ADR-019.** Wenn echter Atari-C-Quellcode 1:1
unverändert kompiliert wird, kommt der direkte Low-Memory-Zugriff zwangsläufig
mit — echte TOS-Apps lesen `_sysbase`/OSHEADER/Cookie-Jar direkt, nicht über
Traps. „Native App nutzt nur die API" gilt nur für Code, den *wir* schreiben,
nicht für geportete Apps. TeraDesk ist der konkrete Gegenbeweis; QED und weitere
werden dieselbe Klasse treffen.

Das ist derselbe Typ Annahme-Umkehrung wie der LP64-App-ABI-Pivot: eine
ursprünglich vernünftige Entscheidung, die die Realität geporteter Apps widerlegt.

## Betroffene Systemvariablen (aus TeraDesks eigenem Code)

| Adresse | Variable | TeraDesk-Fundstelle | Wann |
|---------|----------|---------------------|------|
| `$4F2` | `_sysbase` → OSHEADER (`os_version` @ +2) | `main.c:56` | **Start** (Version-Detection) → aktueller Crash |
| `$420` | `memvalid` | `main.c:57` | Shutdown/Reset |
| `$43A` | `memval2` | `main.c:58` | Shutdown/Reset |
| `$426` | `resvalid` | `main.c:60` | Shutdown/Reset |
| `$42A` | `resvector` | `main.c:59` | Shutdown/Reset |
| `$5A0` | `_p_cookies` (Cookie-Jar), gelesen via `Setexc(0x5A0/4, -1)` | `cookie.c:28,56` | Feature-Detection |

OSHEADER-Feldlayout + Werte, die Apps erwarten: siehe
`docs/api/structures/osheader.md` (44 B; `os_version` `0x0400`, `os_run` →
current PD, `os_kbshift`, `_ETOS`/`_NTOS`-Signatur usw.).

## Prüfauftrag ans Audit

1. **Trägt die Zwei-Welten-Annahme?** Reicht der API-Zugang (BIOS/XBIOS/`Ssystem`)
   für native geportete Apps, oder **müssen** Low-Memory-Sysvar-Area + OSHEADER
   auch für native EL0-Apps angelegt werden? (TeraDesk-Evidenz: müssen.)
2. **Layout in der EL0-VA:** die App liegt ab Entry `0x100` — wo liegt dann eine
   reservierte Sysvar-Area (`$400`–`$5FF`), ohne mit dem App-Image zu kollidieren?
   Muss das per-Prozess-Memory-Layout einen Low-Memory-Bereich reservieren?
3. **OSHEADER-Instanz:** eine reale, kernel-befüllte OSHEADER-Struktur, auf die
   `_sysbase` zeigt — welche Felder mit welchen Werten (`os_version`, `os_run` →
   current-PD-Zeiger, `os_kbshift`, `os_beg`/`os_end`, Signatur)?
4. **Cookie-Jar:** ADR-012 machte den Jar kernel-owned ohne `$5A0`. Braucht der
   direkte `$5A0`-Pfad + `Setexc(0x5A0/4,…)` (BIOS Trap 13 fn 5) trotzdem eine
   Low-Memory-Sichtbarkeit für geportete Apps?
5. **Scope über die Trap-Tabellen hinaus:** der Fix ist primär **Boot-Zeit-
   Befüllung** (Kernel-Init), nicht ein einzelner Trap-Handler. Das Audit von
   BIOS/XBIOS muss diese Init-Ebene mit erfassen, nicht nur die Dispatcher.
6. **Vollständigkeit:** welche weiteren Sysvars (`_hz_200`, `_bootdev`,
   `_v_bas_ad`, drive map, …, `docs/COMPATIBILITY.md#system-variables`) fassen
   typische GEM-Apps direkt an, und welche fehlen aktuell?

## Referenzen

- **Spec:** `docs/api/structures/osheader.md`, `docs/api/bios/_cookie-jar.md`
- **ADRs / Contract:** ADR-012 (`docs/MEMORY.md`), `docs/COMPATIBILITY.md`
  („System variables — compatible how"), ADR-019 (App-Port-Regel)
- **EmuTOS:** `bios/bios.c` (`os_header`-Definition + Low-Memory-Init),
  `include/biosdefs.h:44-63` (OSHEADER-Layout), `bios/startup.S` (Sysvar-Layout)
- **App-Beleg:** TeraDesk `main.c:55-60`, `library/utility/cookie.c:28,56`
- **Fault:** ESR=0x92000004 (Translation fault L0), ELR=0x11814, App-VA-Zugriff
  auf `$4F2`
