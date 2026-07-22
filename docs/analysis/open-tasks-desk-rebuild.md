# Offene Tasks — Desktop 1:1 EmuTOS Rebuild & AES Render-Kampagne

> Stand: 2026-07-22, Branch `desk-rebuild`, HEAD `5b23601`.
> Ersetzt die (getrennten) MCP-Task-Tools — die sind disconnectet, Task-Nummern
> sind nur noch historische Referenzen. Diese Datei ist der Tracker.

Leitprinzip: **1:1 EmuTOS.** Bei jeder Abweichung ist der Default „match it"
(EmuTOS `file:line` zitieren + reproduzieren), nicht „benign, lassen".
Siehe Memory `no-rationalizing-emutos-deviations`, `desktop-1to1-emutos-rebuild` (ADR-022).

Workflow pro Problem: Developer implementiert → **unabhängiger** Tester verifiziert
(nicht dem Dev-Selbstreport trauen) → Lead committet bei PASS → nächstes. Seriell.
Keine Stubs. Worker nehmen keine HW-/Outward-Actions (Flash/Eject = Lead/User).
Root-Cause mit Evidenz VOR dem Fix.

---

## A. AKTIV / als Nächstes

### A1 — #127 Queue-Unification (Bypass raus) — NICHT gestartet, User-greenlit
**Problem:** NewTOS hat zwei Message-Pfade. WM_REDRAW geht über die Queue
(`post_msg` → per-app FIFO), aber **WM_MOVED / WM_TOPPED / WM_SIZED / Menü**
schreiben über `window::intercept` / `emit` **direkt in `pbuff`**
(`tos-api/src/aes/window.rs:1761` / `:1834`) und umgehen die Queue.
→ Nicht 1:1; drop-anfällig, sobald eine Geste mehrere Messages erzeugt.

**EmuTOS-Referenz:** ALLE Inter-Prozess-Messages laufen über **einen** Pfad:
`ap_sendmsg` (`gemwmlib.c:851-863`) → `ap_rdwr(MU_SDMSG)` → `aqueue`/`doq`
→ per-app FIFO `p_queue[QUEUE_SIZE=128]` (`struct.h`), gelesen von `ev_mesag`
→ `ap_rdwr(MU_MESAG)` → dequeue (`gemevlib.c:100-116`). WM_REDRAW-Rect-Union
beim Enqueue (`gemqueue.c:45-56`, hier lokal gelesen). **Kein** Message umgeht die Queue.

**Aufgabe:** Alle AES-generierten Messages über `post_msg` / ap_sendmsg-Stil
routen, den `intercept→pbuff`-Bypass entfernen, sodass `evnt_multi` alles aus
der Queue liefert. Prime-Suspect für die verbleibenden Redraw-Probleme.
**Depends on:** appl_init (erledigt, `1579655` + `5b23601`).

### A2 — Verbleibende Redraw-Probleme diagnostizieren — offen
appl_init hat geholfen („etwas besser"), aber User meldet weiterhin „einiges an
Redraw-Problemen" auf realer HW. **Noch nicht root-caused.** Braucht Präzisierung
vom User: Move? Open? Menü? überlappende Fenster? Maus-Spuren überall/selektiv?
A1 (#127) ist der Hauptverdacht für die Multi-Message-Drop-Klasse, aber nicht
annehmen — erst Symptome einsammeln, dann gegen EmuTOS prüfen. Ggf. deckt A1 einen
Teil ab; der Rest (v. a. A3) ist separat.

### A3 — Maus-Artefakte auf dem Desktop — flagged PLAUSIBLE, nicht untersucht
Cursor-Save-Under vs. direkte Render-Pfade: beim Redraw bleibt der alte
Maus-Cursor als Artefakt stehen. Vom User zweimal gemeldet
(„es gibt Artefakte auf dem Desktop von der Maus"). Separat von A1.
Zu untersuchen: wie EmuTOS den Cursor beim Redraw hidet/restauriert
(`gsx_mcursor` / mouse hide-show um `w_redraw`).

---

## B. Desktop 1:1 Rebuild — restliche Module (#124)

Module 1 (do_wredraw 1:1, `14c28f0`) ist erledigt. Offen:

- **B1 — Modul 2 (Dialoge → App-Pfad):** Desktop-Dialoge über den App-/AES-Pfad
  statt direktem EL1-`objc_draw`; die Modul-2-`redraw_desktop`-Reliance entfernen.
- **B2 — Modul 3 (menu/window/event 1:1):** Menü-/Fenster-/Event-Handling der
  desk/ 1:1 gegen `deskmain.c` / EmuDesk. Überschneidet sich mit A1.
- **B3 — Modul 4 (file/dir/INF/rez 1:1):** File-/Dir-Ops, `.INF`-Handling,
  Resource-Load 1:1; `desksupp.c` `do_aopen`/`do_open` in eine neue
  `docs/api/desktop/desksupp.md` extrahieren.
- **Endgame:** desk/ als ersetzbare **EL0-AES-App** (späte Stufe) — der Grund für
  den ganzen Rebuild (AUTO-Ordner-Desktop darf nicht brechen).

---

## C. Fix B — AES OBJECT/FORM Render (3D-Look), deferred Rest

Groups A/B/C committet (D1/D9, D3/D4/D5, D7). Deferred:
- **C1 — D2:** SMALL 6x6-Font. Deferred **zusammen mit** UI-Scaling (E3), weil
  6x6 bei 1080p Legibility-Frage ist.
- **C2 — D6:** G_USERDEF-Objekte.
- **C3 — D8:** INDIRECT-Objekte (`ob_flags & INDIRECT`).
- Audit-Doc `docs/analysis/aes-object-form-render-audit.md` ist noch **uncommitted**
  (siehe F).

---

## D. QED / cflib App-Pfad

- **D1 — CLIPBRD „cannot create":** QED konnte `CLIPBRD` nicht anlegen obwohl der
  Ordner existiert. Ursache lag im Fattrib-TOS4-vs-TOS1-3-Fork (cflib
  `path_exists`/`fsexists.c:41` erwartet FA_DIR auf Ordnern = TOS4). Fix C
  (Fattrib TOS4-Superset, `8af8c21`) committet. **Auf realer HW re-verifizieren**,
  ob CLIPBRD-Erstellung jetzt durchgeht. Memory `cflib-apps-expect-tos4-semantics`.

---

## E. Geparkt (minor 1:1 Follow-ups + Scaling)

- **E1 — #120:** FL3DOBJ text/box `th` (Feinheit im 3D-Border).
- **E2 — #121:** SHADOWED hittest + Spec.
- **E3 — #122:** Globales UI-Scaling (der „eigentliche Fix" für 1080p-Legibility).
  Vom User bewusst gewählt, dann deferred („jetzt aber nicht notwendig"). Schaltet
  C1 (D2 6x6) frei.
- **E4 — #126:** chkdisabled press-edge / popup.

---

## F. Housekeeping / uncommitted Working-Tree

Aktuell nicht committet (git status):
- `M .claude/agents/{auditor,developer,researcher}.md`, `?? .claude/agents/tester.md`
  — Agent-Definitionen (Tooling, nicht Produkt-Code).
- `M kernel/src/arch/aarch64/exceptions.rs`, `M kernel/src/lib.rs`,
  `?? kernel/src/dbg_qedtrace.rs` — QED-Trace-Debug-Instrumentierung. Klären:
  committen (als `chore(debug):`) oder verwerfen, bevor #127 startet.
- `?? docs/analysis/aes-object-form-render-audit.md` — Fix-B-Audit, sollte mit
  committet werden (gehört zu C).
- `?? sdk/apps/build-teradesk-ilp32.sh`, `?? pictures/` — Build-Script /
  Test-Screenshots. Entscheiden ob ins Repo oder ignoren.

---

## Erledigt (Referenz, Branch `desk-rebuild`, neueste zuerst)

- `5b23601` appl_init füllt aes_global 1:1 (ap_private, ap_3resv)
- `1579655` desk ruft appl_init (Empty-Window Root-Cause)
- `8d77372` per-app FIFO Message-Queue (#125, notwendig aber nicht hinreichend)
- `bb0b6b9` P3: kein Highlight auf DISABLED-Menüitem bei Hover
- `ef9bfa8` P5: WM_REDRAW an uncovered/topped Fenster bei wind_close/WF_TOP
- `14c28f0` Modul 1: do_wredraw 1:1
- `dd44060` Spec-Validierung + ADR-022
- `26a9b7e` P1+P2: Menü save/restore
- `d95ea59` D7 · `3404a18` D3-D5 · `b8c3a9e` D1+D9 (Fix B)
- `8af8c21` Fix C: Fattrib TOS4-Superset
- `ec054a0` Fix A: rsrc_obfix
