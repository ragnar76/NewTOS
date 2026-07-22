# VDI 1:1-Audit — PAKET V1 (control / workstation / dispatch / inquiry)

> Auditor, 2026-07-22. Statisch (kein cargo/QEMU). Ziel-Profil Falcon030/TOS4
> make-512. Vom Lead persistiert (Auditor-Harness verbietet .md-Schreiben).

**Scope:** `v_opnwk/v_opnvwk/v_clswk/v_clsvwk/v_clrwk/v_updwk`, `init_wk`/
Default-Attribute, Handle-Alloc, Trap-#2-Dispatcher (Opcode-Routing, Slot-ABI,
ILP32/LP64), Inquiry-Opcodes (`vq_extnd, vql/vqm/vqf/vqt_attributes, vq_color,
v_get_pixel, vqt_extent/width/name/fontinfo, vq_key_s, vsin_mode, vqin_mode`)
gegen `vdi_control.c`, `vdi_main.c`, `vdi_misc.c`, `vdi_defs.h`.

Ziel-Profil-Fakten (statisch verifiziert): Framebuffer 1920×1080, VDI-Palette
16 Farben (`NUM_COLORS=16`, `kernel/src/vdi_backend.rs:21`) → Falcon/VIDEL-4-Plane-WS.

## Teil 1 — Coverage-Tabelle

| EmuTOS fn | file:line | NewTOS fn (file:line) | Status | 1:1? |
|---|---|---|---|---|
| screen (Dispatcher) | vdi_main.c:121 | dispatch (mod.rs:205) | partial | teilw. (kein jmptab-Count-Preload, kein multifill-Reset, kein CUR_WORK/flip_y) |
| vdi_v_opnwk (1) | vdi_control.c:496 | v_opnwk (workstation.rs:197) | partial | NEIN (SIZ_TAB, DEV_TAB[3][4]) |
| vdi_v_clswk (2) | vdi_control.c:573 | (alias→v_clsvwk, mod.rs:227) | **MISSING** | **NEIN** |
| vdi_v_clrwk (3) | vdi_control.c:599 | v_clrwk (workstation.rs:213) | impl | ja |
| vdi_v_opnvwk (100) | vdi_control.c:407 | v_opnvwk (workstation.rs:140) | partial | NEIN (SIZ_TAB, Handle-Range) |
| vdi_v_clsvwk (101) | vdi_control.c:462 | v_clsvwk (mod.rs:227→workstation.rs:206) | partial | NEIN (kein PHYS-Guard) |
| vdi_vq_extnd (102) | vdi_control.c:626 | vq_extnd (workstation.rs:156) | partial | NEIN (SIZ_TAB, INQ_TAB[14]) |
| v_updwk (4) | jmptb1[3]=v_nop (vdi_main.c:36) | no-op (mod.rs:315 `_ =>`) | impl | ja (No-op beidseitig) |
| init_wk | vdi_control.c:269 | init_wk (workstation.rs:70) | partial | NEIN (ud_patrn, SIZ_TAB) |
| get_vwk_by_handle | vdi_control.c:178 | vwk::resolve (vwk.rs:220) | partial | NEIN (Range 16 vs 64) |
| build_vwk_chain | vdi_control.c:390 | — | **MISSING** | NEIN (line-A only) |
| validate_color_index | vdi_control.c:224 | vwk.rs:146 | impl | ja |
| vdi_vql_attributes (35) | vdi_line.c:156 | vql_attributes (inquiry.rs:20) | impl | ja |
| vdi_vqm_attributes (36) | vdi_marker.c:169 | vqm_attributes (inquiry.rs:32) | impl | ja |
| vdi_vqf_attributes (37) | vdi_fill.c:321 | vqf_attributes (inquiry.rs:43) | impl | ja |
| vdi_vqt_attributes (38) | vdi_text.c:999 | vqt_attributes (inquiry.rs:54) | impl | ja |
| vdi_vq_color (26) | vdi_col.c:845 | vq_color (inquiry.rs:75) | partial | teilw. (flag/req_col=V3) |
| vdi_v_get_pixel (105) | vdi_fill.c:1095 | v_get_pixel (inquiry.rs:112) | impl | ja (Struktur) |
| vdi_vqt_extent (116) | vdi_text.c:1024 | vqt_extent (inquiry.rs:130) | impl | ja (monospace) |
| vdi_vqt_width (117) | vdi_text.c:1062 | vqt_width (inquiry.rs:171) | impl | ja (monospace) |
| vdi_vqt_name (130) | vdi_text.c:1100 | vqt_name (inquiry.rs:191) | partial | NEIN (Element/Name-String) |
| vdi_vqt_fontinfo (131) | vdi_text.c:1137 | vqt_fontinfo (inquiry.rs:202) | impl | ja |
| vdi_vq_key_s (128) | vdi_input.c:110 | vq_key_s (inquiry.rs:121) | partial | NEIN (Maske 0x0f) |
| vdi_vsin_mode (33) | vdi_input.c:118 | vsin_mode (input.rs:25) | impl | ja |
| vdi_vqin_mode (115) | vdi_input.c:154 | vqin_mode (input.rs:43) | impl | teilw. (Out-of-range) |

## Teil 2 — Divergenz-Findings

### AUD-V1-001: `v_clswk` (Opcode 2) fehlt — auf `v_clsvwk` umgebogen, zerstört die physische Workstation
- Severity: **BLOCKER**
- EmuTOS: `vdi_control.c:573-590` (`vdi_v_clswk`: schließt alle virtuellen WS `vwk_ptr[2..]` via `Mfree`, setzt `CUR_WORK=phys`, `timer_exit/vdimouse_exit/esc_exit`; physische WS bleibt offen). Jump-Table `vdi_main.c:34` = `{0,0,vdi_v_clswk}`, distinkt von `jmptb2[1]={0,0,vdi_v_clsvwk}` (101).
- NewTOS: `tos-api/src/vdi/mod.rs:227-231` — `2 | 101 => { if let Some(h)=open_handle(&pb) { workstation::v_clsvwk(h); } }`. Opcode 2 mit 101 zusammengelegt → `vwk::free_handle(handle)` (`workstation.rs:206`, `vwk.rs:238`).
- Divergenz: Opcode 2 wird mit dem physischen Handle 1 aufgerufen (Normalfall Standalone-GEM). NewTOS `free_handle(1)` → `t[0]=Vwk::EMPTY` → physische WS geschlossen; alle VDI-Ops auf Handle 1 danach stilles No-op (Zeichnen stoppt). Zusätzlich offene virtuelle WS NICHT geschlossen (Handle-Leak). EmuTOS macht das Gegenteil.
- 1:1-Fix: Opcode 2 als eigene Fn — Handles 2..N iterieren + `free_handle`, Handle 1 NICHT freigeben, CUR_WORK=physical. Verletzt eigene Spec `docs/api/vdi/workstation/101-v_clsvwk.md:76-88` + `01-v_opnwk.md:193-194`.
- Runtime-verify: yes

### AUD-V1-002: `v_clsvwk` (101) ohne `VDI_PHYS_HANDLE`-Guard — `v_clsvwk(1)` gibt physische WS frei
- Severity: **HIGH**
- EmuTOS: `vdi_control.c:467-469` — `handle = vwk->handle; if (handle == VDI_PHYS_HANDLE) return;`.
- NewTOS: `workstation.rs:206-209` → `vwk::free_handle(handle)` (`vwk.rs:238-245`), kein PHYS-Check; `resolve(1)`→`Some`, `free_handle(1)`→`t[0]=EMPTY`.
- Divergenz: `v_clsvwk(contrl[6]=1)` schließt bei NewTOS die physische WS; EmuTOS = No-op. Widerspricht Spec `101-v_clsvwk.md:62-63,128`.
- 1:1-Fix: `if handle == PHYS_HANDLE { return; }` vor Freigabe.
- Runtime-verify: yes

### AUD-V1-003: `SIZ_TAB[0..3]` = `{0,7,0,7}` statt `text_init`-Metriken `{5,4,7,13}`
- Severity: MEDIUM
- EmuTOS: `vdi_control.c:552` `text_init()`; `vdi_text.c:598-642` → min/max `max_char_width`/`top` über fon6x6/8x8/8x16 ⇒ `{5,4,7,13}`; `init_wk` kopiert SIZ_TAB→PTSOUT (`vdi_control.c:349-352`).
- NewTOS: `workstation.rs:44-48` (`SIZ_TAB_ROM={0,7,0,7}`), unverändert nach PTSOUT (`:125-127`, `vq_extnd` Mode 0 `:172-174`); kein `text_init`.
- Divergenz: `work_out` liefert max char width `PTSOUT[2]=0` (EmuTOS 7). Selbst ein korrekter Port ergäbe `{7,13,7,13}` (fon6x6/8x8 fehlen, V4) — der `0`-Wert bleibt klarer Fehler.
- 1:1-Fix: SIZ_TAB aus vorhandenen System-Fonts berechnen.
- Runtime-verify: yes

### AUD-V1-004: `DEV_TAB[3]/[4]` (Pixelgröße) = `372` statt VIDEL `278`
- Severity: MEDIUM
- EmuTOS: `vdi_control.c:205` `get_pixel_size`; `bios/screen.c:872-876` VIDEL: 1920×1080 ⇒ 278/278. Nie 372.
- NewTOS: `workstation.rs:33-41` `DEV_TAB_ROM[3]=[4]=372`; `init_wk` patcht `[3]/[4]` nicht; kein `get_pixel_size`.
- Divergenz: `INTOUT[3]=[4]=372` statt 278 → falsches Seitenverhältnis für `v_arc/v_circle/v_pieslice`. Intern inkonsistent (`[39]=4096` = VIDEL-4bpp, `[3]/[4]=372` = ST-High).
- 1:1-Fix: `DEV_TAB[3]/[4]=278`.
- Runtime-verify: yes

### AUD-V1-005: `INQ_TAB[14]` (max Vertices) = `256` statt `512`
- Severity: MEDIUM
- EmuTOS: `INQ_TAB_rom[14]=MAX_VERTICES` (`vdi_control.c:91`); `config.h` = 512 für alle Targets.
- NewTOS: `workstation.rs:51` `MAX_VERTICES=256` → `INQ_TAB_ROM[14]`.
- Divergenz: `vq_extnd` Mode 1 `INTOUT[14]=256`; auch realer Polygon-Cap 256 (V2).
- 1:1-Fix: `MAX_VERTICES=512`.
- Runtime-verify: yes

### AUD-V1-006: Handle-Anzahl 16 statt `NUM_VDI_HANDLES ≥ 64`
- Severity: MEDIUM
- EmuTOS: `config.h` NUM_VDI_HANDLES=64 (bzw 128); 1..64.
- NewTOS: `vwk.rs:20` `HANDLE_COUNT=16`; `alloc_handle` 2..=16.
- Divergenz: 17. `v_opnvwk` schlägt fehl (EmuTOS bis 64). Nur per Erschöpfung beobachtbar.
- 1:1-Fix: `HANDLE_COUNT=64`.
- Runtime-verify: yes

### AUD-V1-007: `vq_key_s` (128) ohne `& 0x0f`-Maske — Caps-Lock-Bit (0x10) leckt
- Severity: MEDIUM
- EmuTOS: `vdi_input.c:110-113` `INTOUT[0]=gshift_s()` = `Kbshift(-1) & 0x000f` (verwirft Caps-Lock 0x10).
- NewTOS: `inquiry.rs:121-125` `set_intout(0, keyboard_shift_state())`, keine Maske; `kernel/src/ikbd.rs:150-171` setzt Caps-Lock `0x10`.
- Divergenz: Bei Caps-Lock `INTOUT[0]=0x10` statt 0x00. Stört exakte Modifier-Prüfungen.
- 1:1-Fix: `set_intout(0, (state & 0x0f))`.
- Runtime-verify: yes

### AUD-V1-008: `init_wk` Default-`ud_patrn` = Nullen statt `ROM_UD_PATRN`
- Severity: MEDIUM
- EmuTOS: `vdi_control.c:335-339` kopiert `ROM_UD_PATRN` (16 WORDs, `vdi_fill.c:50-53`) in `vwk->ud_patrn`.
- NewTOS: `vwk.rs:128` `ud_patrn=[0;16]`; `init_wk` setzt es nicht.
- Divergenz: `vsf_interior(FIS_USER=4)` ohne vorheriges `vsf_udpat` füllt bei EmuTOS mit ROM-Muster, bei NewTOS mit Nullen (V2-Visual).
- 1:1-Fix: `init_wk` mit `ROM_UD_PATRN` initialisieren.
- Runtime-verify: yes

### AUD-V1-009: Dispatcher setzt `vwk->multifill=0` pro Nicht-Open-Op nicht zurück
- Severity: MEDIUM
- EmuTOS: `vdi_main.c:145-148` — für jeden Nicht-OPNWK/OPNVWK-Op: `if (vwk->fill_style != 4) vwk->multifill = 0;`.
- NewTOS: `mod.rs:205-317` — kein Reset.
- Divergenz: multifill bleibt nach Style-Wechsel weg von 4 erhalten (EmuTOS löscht pro Op). V2-Fill-Bezug.
- 1:1-Fix: nach Handle-Resolve `if vwk.fill_style != 4 { vwk.multifill = false; }`.
- Runtime-verify: yes

### AUD-V1-010: `build_vwk_chain`/`CUR_WORK` nicht gepflegt
- Severity: MEDIUM
- EmuTOS: `vdi_control.c:390-403` (Vwk-Kette, "Warp9 expects this"), CUR_WORK-Pflege.
- NewTOS: `vwk.rs` kein `next_work`/`CUR_WORK`.
- Divergenz: Nur über Line-A-Zugriff beobachtbar (nicht angeboten) → praktisch unbeobachtbar via VDI-API, aber echte Auslassung.
- 1:1-Fix: bei Line-A-Kompat nachbilden, sonst als bewusste Abweichung dokumentieren.
- Runtime-verify: no

### AUD-V1-011: `vqt_name` (130) ignoriert `INTIN[0]`, liefert konstant "8x16 system font"
- Severity: LOW
- EmuTOS: `vdi_text.c:1100-1134` — Element 1 = font_ring[0]=fon6x6 ⇒ "6X6 system font".
- NewTOS: `inquiry.rs:191-198` — ignoriert INTIN[0], immer FONT_ID=1 + "8x16 system font".
- Divergenz: `vqt_name(1)` liefert falschen Face-Namen. Nur eine Face existiert → kosmetisch.
- Runtime-verify: yes

### AUD-V1-012: `flip_y` (VDI-Trap-`d0`-Rückgabe) nicht modelliert
- Severity: LOW (acceptable deviation, D-080)
- EmuTOS: `flip_y=1` in init_wk/vq_extnd/vqm/vqt_*; gelesen nur `vdi_asm.S:125` als d0.
- NewTOS: `dispatch` gibt `()`; kein Modell. Über flache Struct-ABI unbeobachtbar; D-080 = Reference nicht Contract.
- 1:1-Fix: keiner (nur bestätigen, dass `svc #2` kein irreführendes x0 liefert).
- Runtime-verify: yes (Absicherung)

### AUD-V1-013: `vqin_mode` (115) bei Out-of-range-Device liefert 0 statt Stale-Wert
- Severity: LOW
- EmuTOS: `vdi_input.c:154-176` — kein default, INTOUT[0] unverändert (Stale).
- NewTOS: `input.rs:43-54` — setzt explizit `INTOUT[0]=0`.
- Divergenz: nur bei Invalid-Input; EmuTOS dort undefiniert. NewTOS "sauberer", nicht 1:1. Kein Contract-Bruch.
- Runtime-verify: no

### AUD-V1-014: `DEV_TAB[5]=3` (Font-Höhen) annonciert, aber nur 8x16 vorhanden
- Severity: LOW (V4-Querbezug)
- EmuTOS: `vdi_text.c:661` `DEV_TAB[5]` = Zahl der font_id==1-Größen = 3.
- NewTOS: `workstation.rs:36` `[5]=3` (hartcodiert), aber `font.rs` nur 8x16.
- Divergenz: `INTOUT[5]=3` behauptet 3 Höhen; nur 8x16 vorhanden. Wert stimmt, Font-Abdeckung (V4) nicht.
- Runtime-verify: no

## Teil 2b — Spec-Findings (Route: Researcher)

- **SPEC-V1-A:** `01-v_opnwk.md:221` behauptet `DEV_TAB[39]=16` — falsch; `[39]`=Palette-Size (4096 für VIDEL-4bpp, Code korrekt `workstation.rs:121`). Spec korrigieren.
- **SPEC-V1-B:** `101-v_clsvwk.md:62-63,128` behauptet einen PHYS-Guard, den der Code nicht hat (AUD-V1-002). Spec-Intention korrekt, Code-Behauptung unwahr.

## Zusammenfassung

**Severity:** BLOCKER 1 · HIGH 1 · MEDIUM 7 · LOW 5 · Spec 2. 14 echte Divergenzen
(11 Bugs + 3 acceptable/Querbezug) auf ~24 Fn. CONFIRMED (statisch): 001-009, 011, 013.

**Top-3:** AUD-V1-001 (BLOCKER, v_clswk schließt Screen-Device + leakt vWS),
AUD-V1-002 (HIGH, v_clsvwk(1) gibt physische WS frei), AUD-V1-003 (MEDIUM,
work_out max char width = 0).

**Querbezüge:** ud_patrn/multifill → V2-Fill; SIZ_TAB/DEV_TAB[5]/vqt_* Fonts → V4;
vq_color-flag → V3.
