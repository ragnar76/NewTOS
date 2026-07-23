# AES 1:1-Audit — MASTER (priorisierte Synthese)

> Stand: 2026-07-22, Branch `desk-rebuild`. Quelle: 6 Verhaltens-Audits
> (`p1`–`p6`), Opcode-Matrix (`coverage-matrix.md`), 402-Funktions-Inventar
> (`emutos-function-inventory.md`), Config-Profil (`emutos-config-profile.md`).
> Ziel-Profil: **Falcon030/TOS4, `make 512`, HW-Ausnahme BIOS/XBIOS** (Memory
> `emutos-target-profile-falcon-tos4`) → jede AES-Divergenz/Absenz ist ein
> **echter 1:1-Gap**, außer reine HW oder laufzeit-HW-No-Op.

## 1. Dashboard

| Dimension | Ergebnis |
|---|---|
| AES-Opcodes (Public-API) | **79/79 PRESENT, 0 MISSING** — auf Trap-Ebene fehlt nichts |
| Verhaltens-Findings (P1–P6) | **8 BLOCKER · 15 HIGH · ~29 MEDIUM · ~26 LOW** |
| EmuTOS-Funktionen (intern, 402) | HAS 265 · **NO-COUNTERPART 99** · N/A 38 |
| DISPATCH-ONLY (hartkodiert) | 4 (`appl_tplay`, `appl_trecord`, `appl_yield`, `scrp_clear`) |

**Kernbild:** Die „großen Unterschiede" sind (a) Verhaltens-/ABI-Divergenzen
*innerhalb* vorhandener Funktionen und (b) ganze *interne* Funktionsgruppen ohne
Gegenstück. Die zwei realen HW-Schmerzen des Users sind root-caused:
**Redraw #127** (Queue-Bypass, P2) und **Maus-Artefakte** (Cursor-Semaphor, P5).

## 2. Priorisierte Fix-Backlog (serielle Phase 2)

Cross-gemergt (dieselbe Ursache über mehrere Audits = ein Work-Package).
Workflow je WP: Developer → **unabhängiger** Tester → Lead-Commit.

### WP1 — Queue-Unification #127 · **BLOCKER** · User-Pain #1 (Redraw)
- Merge: **P2-001, P2-002, P2-003** + pid-Kollision **P1-004**.
- WM_CLOSED/MOVED/SIZED/TOPPED (`window.rs:1880 emit`) und MN_SELECTED
  (`event.rs:77 menu_intercept`) umgehen die per-App-FIFO → `queue.pop()`
  überschreibt frische Message → **Fenster schließt nicht / Menü verloren**.
- 1:1: alle AES-Messages über `post_msg`/ap_sendmsg-Pfad; `msg[1]`=Sender-PID;
  `intercept` nur bei angefordertem MU_MESAG; `current_pid()` Kollision (Desktop
  EL1 vs. erste EL0-App beide → pid 1) beheben.

### WP2 — Cursor-Semaphor · **HIGH** · User-Pain #2 (Maus-Artefakte)
- Merge: **P5-002, P5-006, P5-007** + P3-Out-of-scope (ob_draw moff/mon).
- Window-Composite (`compose`/`redraw_clip`/`draw_frame`/`redraw_open`/
  `redraw_vslider`/`redraw_hslider`) klammert nicht mit `gsx_moff`/`gsx_mon` →
  stale Save-under-Sprite → Cursor-Droppings. `graf_mouse` M_OFF/M_ON nicht
  zählend; M_SAVE/M_RESTORE/M_PREVIOUS No-Op.
- 1:1: hide/show-Klammerung um jede Fenster-Neuzeichnung (wie EmuTOS `w_update`
  gemwmlib.c:1011/1047); zählender `gl_moff`-Semaphor; Saved-Form-Slot im VDI-Cursor.

### WP3 — Resource + Object-Spec 1:1 · **2 BLOCKER + 2 HIGH** · blockt ported Apps
- Merge: **P6-001** (rsrc_saddr STUB), **P6-002** (rsrc_gaddr 11/17 R_*-Typen),
  **P3-001** (INDIRECT nie aufgelöst → Crash), **P3-002** (G_USERDEF/ob_user/
  call_usercode fehlen), **P3-006** (G_CICON), **P6-012** (fix_objects
  Exclusion-Logik + CICON-Fixup), Inventar-Gruppe **F** (12 CICON-Fn).
- 1:1: `get_addr` alle R_*-Selektoren + Array-Basen (tedinfo/iconblk/bitblk);
  `rsrc_saddr` echt; INDIRECT-Resolve zentral in `ObjTree::spec`; G_USERDEF-Arm
  mit PARMBLK+ub_code; G_CICON mind. Mono-Fallback + CICON-Kette;
  Relocation auf Ausschluss-Logik.

### WP4 — Form-Manager 1:1 · **2 BLOCKER + 2 HIGH**
- Merge: **P4-001** (form_error Return invertiert + falsche Fehlertexte + n>63),
  **P4-002** (form_do ignoriert start_fld), **P4-003** (form_alert Enter bei
  defbut==0), **P4-004** (find_obj wrap + HIDETREE/DISABLED-Filter), + P4-005..019
  (Grammatik-Limits, DEFAULT-Optik, fm_show/eralert, fm_own, XOR-Anim).

### WP5 — Object-Render-Fidelity 1:1 · **P3 Rest** (11 MED)
- P3-003 (objc_change Clip int_in[2..5] + Early-Out), P3-004 (WHITEBAK),
  P3-005 (Icon SMALL-Font/Offset), P3-007 (ob_type & 0x00ff), P3-008 (EDSTART),
  P3-009/010 (Edit Full-Field/scan_to_end), P3-011 (Writing-Mode), P3-012
  (NICELINES-Separator), P3-013 (Zero-Clip-Konvention), P3-022 (EXTENDED_OBJECTS
  Unterstreichung).

### WP6 — Window/Graf 1:1 · **1 BLOCKER + Rest P5**
- P5-001 (graf_slidebox Return-Slot — Elevator tot, **BLOCKER**), P5-003
  (wind_delete deckt nicht auf), P5-004 (Slider-Drag/Track WM_HSLID/VSLID/
  WA_*PAGE), P5-005 (wm_update Semaphor-Stub), P5-010 (WF_COLOR/DCOLOR/OWNER),
  P5-011 (WM_UNTOPPED, merge P2-012 + Inventar `perform_untop`), P5-008/009/013.

### WP7 — Fileselector 1:1 · **P6 fsel** (2 HIGH)
- P6-003 (keine Sortierung), P6-004 (Ordner bei Maske versteckt), P6-007
  (`.`/`..`), P6-008 (Titel ignoriert), P6-013 (FCLSBOX-Up-Nav), P6-014
  (128-Cap) + Inventar-G (`fs_add`/`fs_comp`/`fs_nscroll`/`centre_title`).

### WP8 — Appl/Shell/Scrap 1:1 · **P1 + P6-shel**
- P1-001 (appl_find matcht nie → `p_nameit` fehlt, Inventar-G), P1-005
  (appl_write 16-B-Trunkierung), P1-002/003 (appl_yield/tplay Return),
  P6-005 (shel_write-Launch nie ausgeführt), P6-006 (shel_read Producer),
  P6-009 (shel_find 2/4 Schritte + `sh_path`), P6-015 (scrp_clear STUB).

## 3. Die 99 NO-COUNTERPART — Triage

| Kategorie | #Fn | Einordnung |
|---|---|---|
| **Echte 1:1-Gaps (jetzt, in WP3–WP8 eingearbeitet)** | ~26 | CICON (12), G_USERDEF (3), p_nameit + pd-Namen (4), fm_show/eralert (2), fsel-Sort/Nav (4), scan_to_end/ob_stfn (2), is_dashes, wm_update |
| **Kooperativer Multi-Prozess-Kern (BASE, Phase 2b)** | 44 | gemasync (8) + gemdisp (7) + gemflag (4) + geminput (21) + gemctrl (4). Kooperativer AES-Scheduler: mehrere AES-Prozesse parken/wecken/canceln. **Teil des treuen EmuTOS** (Voraussetzung für Accessories) → in 2b implementieren, nicht Epic. Größter Einzelposten, berührt Single-Tasking-Bring-up. |
| **Desk-Accessories (.ACC) (BASE, Phase 2b)** | 13 | Laden/Starten/Owner-Routing. Hängt am kooperativen Kern. Falcon/TOS4-Feature → gehört ins Grundgerüst, implementieren sobald der Kern (2b) steht. |
| **Desktop-Shell-Layer** | 8 | EMUDESK.INF-Parsing, `sh_main`, Modus-Umschaltung. Gehört zur `desk/`-1:1-Rebuild (ADR-022), separate Kampagne, außerhalb `tos-api/aes`. |
| **GSX/VDI-Transport + Gadget-Tree** | ~8 | N/A-nah: direkte `RasterBackend`-Architektur bzw. geometrisches `gadget_at` statt `gl_awind`-Baum. Endergebnisse sind HAS. Kein Fix. |

## 4. Grundgerüst vs. Weiterentwicklung (User-Entscheidung 2026-07-22)

**Korrektur der ursprünglichen „Option A/B"-Framing:** Der kooperative
Multi-Prozess-Kern (44 Fn) + Accessories (13 Fn) sind **Teil des EmuTOS-
Grundgerüsts**, KEINE „Weiterentwicklung". EmuTOS *ist* kooperativ multitaskend
(Desktop + bis zu 6 Accessories nebeneinander, ein Haupt-Programm). Ein treues
EmuTOS enthält sie also → sie gehören in **Phase 2b (fehlende Funktionen
implementieren)**, nicht in ein zurückgestelltes Epic.

- **„Grundgerüst" (jetzt umsetzen, 2a+2b):** alle Bugs + ALLE fehlenden
  EmuTOS-Funktionen inkl. kooperativem Kern + Accessories + CICON. Akzeptanz:
  **läuft sauber & fehlerfrei auf dem echten Pi.**
- **„Weiterentwicklung" (erst NACH sauberem Grundgerüst auf dem Pi):** alles
  *jenseits* von EmuTOS — FreeMiNT-Preemption / mehrere gleichzeitige Haupt-Apps
  (Memory `freemint-extension-direction`), native/LP64-Extensions, das
  desk-als-EL0-App-Endgame (ADR-022).

Abgrenzung im Kern: EmuTOS-Niveau = 1 Haupt-App + Accessories + Desktop
kooperativ (das ist Grundgerüst, kompatibel mit `singletasking-app-owns-full-screen`
= nur EIN Haupt-Programm). Mehrere gleichzeitige *Haupt-Apps* / Preemption =
Weiterentwicklung. Der kooperative Kern (2b) ist der größte Einzelposten und
berührt die Single-Tasking-Bring-up-Annahme in `event.rs` — als eigene
Teilphase mit realer-Pi-Verifikation fahren.

## 5. Korrekturen an bestehenden Audits

- **P6 (`p6-...md:13,215`):** `CONF_WITH_SHUTDOWN` ist **=1** (kompiliert), nicht
  „wegkompiliert". No-Op entsteht laufzeit über `can_shutdown()`. Ergebnis
  (ehrliches No-Op) bleibt, Begründung korrigieren. Siehe `emutos-config-profile.md`.
- **Deep-Check-Lücken aus Config-Profil:** `CONF_WITH_EXTENDED_OBJECTS`
  (P3-022 deckt es teilweise) und `CONF_WITH_LOADABLE_CURSORS` (Cursor-Init,
  ungeprüft) — in WP5 bzw. WP2 aufnehmen.

## 6. Reihenfolge & Akzeptanz

> **RE-SEQUENCING (User, 2026-07-22, Memory `fix-vdi-before-aes-bottom-up`):**
> **Zuerst das VDI 1:1 sauber, dann diese AES-Fixes.** AES setzt auf VDI-Primitiven
> auf — AES auf kaputtem VDI zu fixen erzeugt nur Kompensations-Hacks. Der VDI-Audit
> (`docs/analysis/vdi-audit/`) + VDI-Master laufen zuerst durch den Fix-Loop.
> **Alle Findings werden gefixt**, nicht nach QED-Sichtbarkeit triagiert (QED = nur
> Bug-Indikator). WP1 (#127) war reines AES-Message-Routing ohne VDI-Bezug → bleibt
> gültig/committet. WP2ff folgen NACH dem sauberen VDI. Die untenstehende WP-Ordnung
> ist damit Dependency-/Layer-basiert, nicht QED-Pain-basiert.

**Phase 2a — Bugs (WP1–WP8):** WP1 (#127, läuft) → WP2 (Cursor) — beide
adressieren die realen HW-Pains → WP3 (Resource/Object-Blocker, blockt
QED/TeraDesk) → WP4 (Form) → WP6 (graf_slidebox-Blocker + Window-Rest) →
WP5 (Object-Render) → WP7 (Fsel) → WP8 (Appl/Shell inkl. pid-Kollision P1-004).

**Phase 2b — fehlende Funktionen:** ~26 echte Einzel-Gaps (CICON, G_USERDEF,
p_nameit, fsel-Sort, fm_show/eralert …), dann der kooperative Kern +
Accessories (größter Posten, eigene Teilphase).

### Fortschritt

- **WP1 (#127 Queue-Unification) — implementiert + QEMU-verifiziert (2026-07-22).**
  `window.rs`/`event.rs`: Bypass entfernt, WM_*/MN_SELECTED via `post_to_owner`→
  `pd::post_msg` in die Owner-FIFO, `msg[1]`=Sender-PID, MU_MESAG nur aus Queue-Pop.
  Tester GRÜN: Build beide Boards, P5/P6-FIFO-Selftests PASS, Screendump kein
  Regress, Boot ok. **Real-Pi-offen:** die 3 interaktiven Klick-/Menü-Repros
  (QEMU hat keinen Maus-Input) → Meilenstein-Flash nach WP1+WP2.
  Offener ABI-Follow-up: `MN_SELECTED` `msg[5..7]` (Tree-Pointer in 2 WORDs,
  MENU_EXTENSION) — `lp64-tos-struct-trap`-Familie, bleibt 0.
- **WP2 (Cursor-Semaphor) — grün (2026-07-23, `f3878eb`).** P5-002/006/007:
  Redraw-Klammerung mit zählendem cursor_hide/show, graf_mouse M_OFF/M_ON
  zählend, M_SAVE/RESTORE/PREVIOUS. Tester 11/11 (Dropping-Fix bestätigt).
- **WP3 (Resource+Object) — grün.** object (`e745175`): INDIRECT/G_CICON-mono/
  G_USERDEF-PARMBLK (EL0-ub_code-Upcall abgesplittet). rsrc (`cf6f3e6`):
  rsrc_gaddr 11 R_*-Typen + NRSC v5/v6, rsrc_saddr, Relocation-Ausschluss
  (voller CICON-Farbsupport = eigenes WP, mkrsc.py erzeugt keine CICONs).
- **WP4 (Form) — grün für die implementierbaren Teile.** WP4a (`77ae9c4`):
  form_do/form_error/find_obj/form_keybd. WP4b (`9e18d0a`): Alert-Grammatik+Optik
  (Icons byte-exakt gem.rsc). WP4c-1 (`9b7278e`): form_dial FMD_FINISH pt-Rect +
  distanz-basierte Animation + fq/Glocke.
  **Dependency-gated, verschoben (kein Stub jetzt):**
  - **P4-011 (fm_own)** → nach WP6: braucht das echte `wm_update` (P5-005);
    der Screen-Lock/Menütree-Hold sitzt auf wm_update auf.
  - **P4-012 (fm_show/eralert)** → zur Basis-Disk-Error-Arbeit: `ETV_CRITIC`
    (bios/vectors.rs) ist definiert, wird aber NICHT aufgerufen — der
    rwabs-Disk-Error-Pfad triggert den Critical-Vektor nicht, `eralert` hätte
    keinen Aufrufer. Erst rwabs→etv_critic→Handler verdrahten, dann eralert/
    fm_show (drive-letter AL00CRT…AL05CRT, Retry/Cancel).

**Akzeptanz-Gate (durchgehend): sauber & fehlerfrei auf dem echten Pi.**
Loop je WP: Developer → unabhängiger Tester (Build + QEMU-Selftest/Screendump,
soweit möglich) → Lead-Commit. Realer-Pi-Check an Meilensteinen (nach WP1+WP2:
Redraw+Cursor; nach 2a: Gesamt-Regression; nach 2b: voller Clean-Run) — **User
flasht** (Worker nehmen keine HW-Actions, Memory `workers-no-hardware-or-outward-actions`).
Erst wenn 2a+2b auf dem Pi sauber laufen → Weiterentwicklung (§4).
