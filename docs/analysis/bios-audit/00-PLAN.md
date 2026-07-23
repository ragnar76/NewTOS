# BIOS + XBIOS Audit — PLAN (Tasks-Definition)

> Stand: 2026-07-23, Branch `desk-rebuild`. Nächste Phase nach der AES-Bug-Fix-
> Phase (siehe `../open-tasks-post-2a.md`). Prinzip wie VDI/AES: 1:1 gegen EmuTOS,
> Default „match it", Developer → unabhängiger Tester → Lead-Commit, keine Stubs.
> Ziel-Profil Falcon030/TOS4 make-512, HW-Ausnahme = RPi4-HAL. Roadmap-Memory
> `post-2a-roadmap-bios-freemint`.
>
> **Noch NICHT gestartet** — dies ist die Task-Definition. Vor der Ausführung von
> BB2 (Memory-Modell) klären wir das Speicher-Layout final (User: ST-RAM fest
> 8 MB, TT-RAM Rest, rein logisch da kein Shifter).

## Warum BIOS zuerst (vor dem kooperativen Kern)

Bottom-up: der kooperative Multi-Prozess-Kern, die OS-Version, das Memory-Modell
und praktisch jede geportete App sitzen auf der **BIOS/Sysvar/Boot-Init-Ebene**.
Das Audit muss die **Boot-Zeit-Befüllung** (Kernel-Init) mit erfassen, nicht nur
die Trap-Dispatcher — der zentrale Befund ist ein Init-Gap, kein Handler-Gap.

## Der auslösende, blockierende Befund

`../sysvar-osheader-lowmem-gap.md`: NewTOS legt die Low-Memory-Systemvariablen
(`$400`–`$5FF`) + den OSHEADER (`_sysbase` @`$4F2`) + den `$5A0`-Cookie-Jar
**nicht** für native EL0-Apps an — bewusst als „m68k-emulator-only" deferrt. Die
„Zwei-Welten-Annahme" (native → API, m68k → Low-Memory) **kollidiert mit ADR-019**
(1:1-Kompilat): echte TOS-Apps fassen diese Adressen direkt an. TeraDesk crasht
beim `os_version`-Check ($4F2); QED & weitere treffen dieselbe Klasse.

## Audit-Work-Packages (bottom-up)

### BB1 — Boot-Init: Low-Memory-Sysvars + OSHEADER + Cookie-Jar (BLOCKER, zuerst)
Der Kern-Fix. Kernel-Boot-Zeit-Befüllung für native EL0-Apps.
- **Prüfen (aus dem Gap-Doc):** Trägt die Zwei-Welten-Annahme? (Evidenz: nein.)
  Layout der Sysvar-Area in der EL0-VA (App ab Entry `0x100` — reservierter
  Bereich `$400`–`$5FF` ohne Kollision, per-Prozess). OSHEADER-Instanz (real,
  kernel-befüllt): `os_version`=**0x0404** (TOS 4.04, Falcon), `os_run`→current-PD,
  `os_kbshift`, `os_beg`/`os_end`, `_ETOS`/`_NTOS`-Signatur. Cookie-Jar-$5A0-
  Sichtbarkeit + `Setexc(0x5A0/4,…)`.
- **Betroffene Sysvars (TeraDesk-Beleg):** `$4F2` _sysbase, `$420` memvalid,
  `$43A` memval2, `$426` resvalid, `$42A` resvector, `$5A0` _p_cookies. Zusätzlich
  auditieren (typische GEM-App-Direktzugriffe): `_hz_200`, `_bootdev`, `_v_bas_ad`,
  drive map, … (`docs/COMPATIBILITY.md#system-variables`).
- **EmuTOS:** `bios/bios.c` (os_header + Low-Memory-Init), `include/biosdefs.h:44-63`
  (OSHEADER-Layout), `bios/startup.S` (Sysvar-Layout). **Spec:**
  `docs/api/structures/osheader.md`, `docs/api/bios/_cookie-jar.md`.
- Nebeneffekt: liefert die TOS-Version 0x0404 (offener Wert von GEMDOS-0x3000-
  Commit `e162c62`).

### BB2 — Memory-Modell: STRAM/TTRAM + Mxalloc + Memory-Descriptors
- **Entscheidung (User 2026-07-23):** ST-RAM = **fest 8 MB**, TT-RAM = **gesamter
  restlicher nutzbarer Speicher**. **Rein LOGISCHE** Pool-Unterscheidung (kein
  Shifter → keine Video-DMA-Bindung; `_v_bas_ad` meldet frei den RPi4-Framebuffer).
- **Umsetzen/auditieren:** GEMDOS `Malloc`(0x48)→ST-RAM-Pool, `Mxalloc`(0x44)→ST
  oder TT je Mode-Flag; Memory-Descriptors (MD/MPB, `Getmpb` BIOS fn 0),
  OSHEADER `os_beg`/`os_end` spannen den ST-RAM-Bereich. Verwandt Memory
  `el0-app-memory-must-be-mapped`. Referenz `docs/analysis/emutos-memory-model.md`.

### BB3 — BIOS-Dispatcher (Trap #13) gegen EmuTOS
Funktion-für-Funktion auditieren (vorhandene + Boot-Init): `Getmpb`(0),
`Bconstat`(1), `Bconin`(2), `Bconout`(3), `Rwabs`(4) **+ etv_critic-Aufruf bei
Disk-Error** (schließt Deferral P4-012/eralert auf), `Setexc`(5), `Tickcal`(6),
`Getbpb`(7), `Bcostat`(8), `Mediach`(9), `Drvmap`(10), `Kbshift`(11).
- **EmuTOS:** `bios/*.c`. **Spec:** `docs/api/bios/`.

### BB4 — XBIOS-Dispatcher (Trap #14) gegen EmuTOS
Falcon/TOS4-relevante Funktionen + HW-Adapter (Screen/VIDEL, IKBD, Sound, Timer,
Cookie/Ssystem). HW-nahe Funktionen (Shifter/MFP/YM2149) sind Projekt-Non-Goals →
als HW-Absenz dokumentieren, nicht 1:1 emulieren; die **beobachtbare API-Ebene**
(Rückgaben, Modus-Queries) aber matchen. FreeMiNT-`Ssystem` (Memory
`freemint-extension-direction`) hier mit einplanen.
- **EmuTOS:** `bios/*.c` (xbios), Falcon-VIDEL. **Spec:** `docs/api/xbios/`.

## Querschnitt: ADR-Konflikt dokumentieren

Die „Zwei-Welten-Annahme" (ADR-012, COMPATIBILITY.md, osheader.md-Notes) wird von
ADR-019 (1:1-App-Port) widerlegt — dieselbe Klasse wie der LP64-ABI-Pivot. Das
Audit muss die betroffenen ADRs/Contract-Zeilen **korrigieren** (nicht nur die
Implementierung), analog zur GEMDOS_VERSION/D-083-Behandlung.

## Reihenfolge & Gate

**BB1 (Boot-Init/OSHEADER — unblockt Ports) → BB2 (Memory-Modell) → BB3 (BIOS) →
BB4 (XBIOS).** Jedes Paket: Developer → unabhängiger Tester (isolierte Worktree-
Verifikation solange parallele Fremd-Edits im Baum) → Lead-Commit. Akzeptanz:
geportete Apps (TeraDesk/QED) starten ohne Sysvar/OSHEADER-Crash; sauber auf dem
echten Pi.

## Referenzen
- Auslöser/Detail: `../sysvar-osheader-lowmem-gap.md`
- Memory-Modell: `../emutos-memory-model.md`
- Spec: `docs/api/structures/osheader.md`, `docs/api/bios/`, `docs/api/xbios/`,
  `docs/api/bios/_cookie-jar.md`
- Contract/ADRs: `docs/COMPATIBILITY.md`, ADR-012/019 (`docs/MEMORY.md`)
- EmuTOS: `bios/bios.c`, `bios/startup.S`, `include/biosdefs.h`
