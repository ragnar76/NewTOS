# Offene Tasks — Stand nach der AES-Bug-Fix-Phase (Phase 2a)

> Stand: 2026-07-23, Branch `desk-rebuild`, HEAD `884dcc1`.
> Löst `open-tasks-desk-rebuild.md` (Stand `db6052d`, veraltet) als aktueller
> Tracker ab. Leitprinzip unverändert: **1:1 EmuTOS**, Default „match it"
> (Memory `no-rationalizing-emutos-deviations`). Workflow: Developer →
> unabhängiger Tester → Lead-Commit, seriell, keine Stubs, isolierte Worktree-
> Verifikation solange parallele Fremd-Edits im Baum liegen.

## 0. Erledigt (committet & je Paket grün getestet)

- **VDI: 1:1 komplett** — Truecolor-Fundament (TC1–TC5) + V1–V6 + Coverage.
  Gate `docs/analysis/vdi-audit/00-GATE.md` 🟢. Commits `727b33c`…`796faf1`.
- **AES Phase 2a (Bug-Fixes):**
  - WP1 #127 Queue-Unification (Vorsession)
  - WP2 Cursor-Semaphor `f3878eb`
  - WP3 Resource+Object `e745175` (object) + `cf6f3e6` (rsrc, NRSC v5/v6)
  - WP4 Form: `77ae9c4` (core) + `9e18d0a` (alerts) + `9b7278e` (form_dial) +
    `71d06e2` (fm_own)
  - WP6 Window/Graf: `4e0a36b` (slider/graf_slidebox) + `8f0671f` (lifecycle) +
    `0e735f4` (wm_update/WM_UNTOPPED/WF_COLOR)
  - WP5 Object-Render: `f0ecefd` (dispatch/clip) + `ec29d3c` (edit-field)
  - WP7 Fileselector: `15bbdb8` (listing) + `579ec5d` (UI) + `6cc7315`
    (gem.rsc embed + disk override)
  - WP8 Appl/Shell/Scrap: `3d8be6f` (appl) + `884dcc1` (shell/scrap)
  - GEMDOS-Version 0x3000 `e162c62`, D-083-Entscheidung `ff85fa5`

## 1. Offen — AES Phase 2a (letztes sichtbares Paket)

### WP6d — sichtbares TOS4-Look-Bündel (bewusst zuletzt, ändert den Desktop)
Zurückgestellt auf User-Wunsch („Look stabil halten während der HW-Tests"; die
sichtbare Änderung als EIN prüfbarer Block):
- **P5-013** Frame-Geometrie: Bänder 18 → 22/19 px (3D/TOS4-Default) in `borders`
  **und** `draw_frame`. Ändert WF_WORKXYWH + verschiebt die WP6a-Slider.
- **P3-004** WHITEBAK-Icon-Halos (gr_gicon Masken-/Text-BG-Skip)
- **P3-005** Icon-Char/Label in SMALL-Font + ib_xicon/yicon-Offset
- **P3-011** Text-Writing-Mode-Bit (MD_REPLACE statt immer transparent)
- **P3-012** NICELINES Menü-Separator (disabled all-dashes → durchgezogene Linie)
- **P3-022** EXTENDED_OBJECTS G_STRING-Unterstreichung (WHITEBAK+0xFF00)
- **WF_COLOR-Render-Konsum**: `draw_frame` liest die per-Window-Farbwörter (WP6c
  legte nur den Round-Trip-State an) — gehört in denselben Frame-Render-Umbau.

## 2. Deferrals — brauchen Infrastruktur/Entscheidung (kein Stub jetzt)

- **P4-012** fm_show/eralert (Drive-Letter-Critical-Alerts, Retry/Cancel) →
  braucht den `etv_critic`-Aufruf: `ETV_CRITIC` ist definiert (bios/vectors.rs),
  wird aber nie aufgerufen; der rwabs-Disk-Error-Pfad triggert ihn nicht. Erst
  rwabs→etv_critic→Handler verdrahten (BIOS-Disk-Error), dann eralert.
- **P6-005** shel_write(SHW_EXEC)-Launch-Konsum → post-Pterm-Chaining gehört in
  den desk/-Shell-Loop + braucht isgem-GUI/Konsole-Routing. Shell-Loop/koop.-Kern.
- **EL0→EL1-Upcalls** (Signal-Infrastruktur, eigener Task):
  - vex-Vektoren aus dem Maus-ISR in eine EL0-App (V6)
  - G_USERDEF `ub_code`-Callback in eine EL0-App (WP3; same-privilege/EL1-Fall
    ist umgesetzt, EL0-Ziel deferred)
- **Voller CICON-Farbsupport** — mkrsc.py erzeugt keine CICONs (zeroet G_CICON
  ob_spec); Mono-Fallback ist umgesetzt (WP3). Braucht CICON-Emission in mkrsc.py
  + fixup_all_ciconblks/transform_all_cicons.
- **Rotierter v_justified-Render** (chup≠0 über den V4b-Rotations-Blit) — VDI,
  kein katalogisiertes Finding, eigener Task.

## 3. Offen — AES Phase 2b (fehlende Funktionsgruppen, „Grundgerüst")

Aus `aes-audit/00-MASTER.md` §3/§4. Ein *treues* EmuTOS enthält sie → Grundgerüst,
nicht Weiterentwicklung.
- **Kooperativer Multi-Prozess-Kern** (44 Fn: gemasync/gemdisp/gemflag/geminput/
  gemctrl) — parken/wecken/canceln mehrerer AES-Prozesse. Größter Posten, berührt
  die Single-Tasking-Bring-up-Annahme in event.rs.
- **Desk-Accessories** (.ACC, 13 Fn) — Laden/Starten/Owner-Routing, hängt am Kern.
- **Desktop-Shell-Layer** (8 Fn: EMUDESK.INF, sh_main, Modus-Umschaltung) —
  gehört zur desk/-1:1-Rebuild (ADR-022).
- ~26 echte Einzel-Gaps (CICON-Farbe, p_nameit erledigt in WP8a, fsel-Sort
  erledigt in WP7a, is_dashes …).

## 4. BIOS + XBIOS Audit — ✅ ANALYSE ABGESCHLOSSEN, Fixes offen

Bottom-up: der kooperative Kern + praktisch alles sitzt auf der BIOS/Sysvar-Ebene.

**Analyse fertig (committet):**
- Researcher-Spec: `docs/api/bios/_boot-init.md`, `docs/api/bios/04-rwabs.md`,
  `docs/api/structures/osheader.md`.
- BIOS-Audit: `bios-audit/bios-findings.md` + `00-DASHBOARD.md` (B1 BLOCKER,
  H1 HIGH, M1–M4 MED, L1–L3 LOW, S1–S3 Spec/Doc).
- Volles Funktionsinventar: `emutos-full-function-inventory.md` (1030 Fn).
- **Machbarkeits-Triage: `bios-audit/bios-xbios-feasibility-triage.md`** —
  BIOS 12/12 = I; XBIOS meist bereits korrekt (Screen/Serial/Uhr = I, Floppy/PSG
  = A, MFP/TT-Shifter/unimpl = X). **Entscheidungen (User):** VIDEL → X/deferred
  (bei App-Bedarf), Audio (PSG/DMA-Sound/DSP) → HW-Absenz jetzt + echtes Audio
  spätere Phase, Supexec → I via EL1-Trampolin deferred, Kbd-XBIOS → I mit
  Keyboard-Phase, Nvmaccess → A. **Kein neuer XBIOS-Arbeitsanfall jetzt.**

**Offen — BIOS-Fixes (der eigentliche nächste Schritt):**
1. **B1 BLOCKER-Machbarkeitsanalyse** (vom User „nach dem Audit" terminiert →
   jetzt): Low-Memory-Sysvars (`$400`–`$5FF`) + OSHEADER (`$4F2`) + Cookie-Jar
   (`$5A0`) je Prozess in die EL0-VA an klassischen Adressen mappen (App lädt ab
   ~`0x100`) — **technisch machbar?** Wenn ja → umsetzen; wenn nein → App-Patch
   notieren (User-Direktive). Liefert TOS-Version 0x0404. `sysvar-osheader-
   lowmem-gap.md`. Verknüpft mit BB2 (STRAM/TTRAM 8 MB / Rest, rein logisch).
2. **H1** etv_*-Dispatch (etv_term im Pterm-, etv_critic im Rwabs-Fehler-, etv_timer
   im Tick-Pfad; Boot-Defaults echt-callable).
3. **M1–M4** Getbpb-FS-Härtung + Rwabs→etv_critic.

Danach: kooperativer Kern FreeMiNT-extensibel (references/freemint pinnen) +
Multi-Core-Vorbereitung für die SMP-Zukunft.

## 5. Nur-HW-Restverifikation (real Pi4)

Interaktive Pfade, in QEMU nicht fahrbar (Input + C:-Storage nicht gleichzeitig):
Fileselector-Klick/FCLSBOX/OK-Cancel, Menü-während-Dialog, v_locator REQUEST,
v_string>256, Live-TOUCHEXIT-Doppelklick, Elevator-Drag H-Achse, appl_find durch
eine zweite laufende App, shel_read/shel_find aus einer echten EL0-App. →
`docs/api/desktop/_hw-test-checklist.md`.

## 6. Akzeptanz-Gate

Durchgehend: **sauber & fehlerfrei auf dem echten Pi** (Memory
`base-emutos-clean-on-pi-before-extension`). User flasht an Meilensteinen.
Erst wenn Basis (Phase 2a + 2b + BIOS) auf dem Pi sauber läuft → FreeMiNT-
Weiterentwicklung.
