# VDI 1:1-Audit — PAKET V5 (raster / blit / backend)

> Auditor, 2026-07-22. Statisch. Ziel-Profil Falcon030/TOS4 make-512 (Truecolor).
> Vom Lead persistiert (Auditor-Harness verbietet .md-Schreiben).

**Scope:** `raster.rs` (vro_cpyfm/vrt_cpyfm/vr_trnfm/Memory-MFDB-Copy),
`backend.rs` (RasterBackend-Trait) + reale Impl `kernel/src/vdi_backend.rs`
gegen `vdi_raster.c` (BitBlt-Logik, nicht asm-Opcode). MFDB-ABI, 16 Schreibmodi,
Screen-Erkennung (fd_addr==NULL), transparent-Copy fg/bg, PAT_FLAG (D-082),
Save/Restore (D-092), Clipping, Overlap, vr_trnfm-Transpose.

## Coverage-Tabelle

| EmuTOS fn | file:line | NewTOS fn (file:line) | Status | 1:1? |
|---|---|---|---|---|
| `vdi_vro_cpyfm` (109) | vdi_raster.c:1470 | vro_cpyfm (raster.rs:266) | impl | ja* (V5-004) |
| `vdi_vrt_cpyfm` (121) | vdi_raster.c:1492 | vrt_cpyfm (raster.rs:215) | impl | **nein** (V5-001,005) |
| `vdi_vr_trnfm` (110) | vdi_raster.c:269 | vr_trnfm (raster.rs:413) | partial | **nein** (V5-002,003) |
| `cpy_raster` (shared) | vdi_raster.c:1330 | inline vro/vrt | impl | ja (vro; vrt=001) |
| `vro_cpyfm16` (TC-Kern) | vdi_raster.c:975 | raster_op+Copy (vdi_backend.rs:432) | impl | ja (16 Ops) |
| `vrt_cpyfm16` (TC-Kern) | vdi_raster.c:1226 | blit_mono+combine (vdi_backend.rs:392) | impl | ja (4 MD) |
| `vr_trnfm16` (16-bit) | vdi_raster.c:190 | — | **MISSING** | nein (002) |
| `bit_blt`/`do_blit` | vdi_raster.c:424 | copy_rect/mem_* | impl (chunky-reimpl) | ja (Logik) |
| `setup_info` (Screen-MFDB) | vdi_raster.c:908 | Mfdb::is_screen (raster.rs:71) | impl | ja |
| `setup_pattern` (D-082 dead) | vdi_raster.c:790 | weggelassen (raster.rs:281) | impl (inert) | ja |
| MFDB-Struktur | vdi_raster.c:166 | Mfdb::from_ptr (raster.rs:71) | impl | ja (LP64 & ILP32) |
| `bb_save`/`bb_restore` (AES) | gemgsxif.c:320 | bb_save/restore (raster.rs:384) | impl | ja (D-092 geschlossen) |
| `linea_raster`/`linea_blit` | vdi_raster.c:1508 | — | n/a (kein Line-A) | n/a |

\* vro_cpyfm 1:1 für 32-bpp-chunky (inkl. bb_save/restore); Abweichung nur nicht-32bpp-Memory-MFDB (004).

## Findings

### AUD-V5-001: `vrt_cpyfm` strippt `PAT_FLAG` (bit4) nicht → legaler Modus `MD_x|PAT_FLAG` = stiller No-Op
- Severity: MEDIUM · CONFIRMED · Bug
- EmuTOS: `cpy_raster` Range-Check `mode & ~0x1f` + `mode &= ~PAT_FLAG` vor MD-Switch (vdi_raster.c:1341,1397).
- NewTOS: `vrt_cpyfm` `match intin(0){1..4=>…,_=>return}` (raster.rs:233), roh, kein Strip. `INTIN[0]=MD_TRANS|PAT_FLAG=18` fällt auf `_=>return` → nichts geblittet. Inkonsistent zu `vro_cpyfm` (strippt korrekt, raster.rs:277). Schlimmer als D-082 (Kopie soll trotzdem passieren).
- 1:1-Fix: `if intin(0) & !0x1F != 0 { return }; mode &= !0x10` vor 1-4-Mapping. Runtime-verify: yes (Selbsttest deckt vrt 0-5, nicht 17-20).

### AUD-V5-002: `vr_trnfm` Truecolor-Zweig (`vr_trnfm16`, nplanes>8) fehlt → Ziel-Profil falsche Ausgabe
- Severity: MEDIUM · PLAUSIBLE · Bug/Absenz
- EmuTOS: `if (TRUECOLOR_MODE && src->fd_nplanes>8) vr_trnfm16(...)` (vdi_raster.c:281), Kern :190-256.
- NewTOS: raster.rs:413-506 kein TC-Zweig; behandelt 16 „Planes" als planaren 1-8-Plane-Transpose → Datensalat. Spec nennt „out of bring-up scope", aber Falcon/TOS4-Ziel braucht den Pfad.
- 1:1-Fix: vr_trnfm16-Äquivalent (Standard↔Device-TC, angepasst an 32-bpp-chunky), gated nplanes>8. Runtime-verify: yes.

### AUD-V5-003: `vr_trnfm` liefert planar, `vro_cpyfm` erwartet chunky → `trnfm→cpyfm`-Pipeline bricht
- Severity: MEDIUM · PLAUSIBLE · Design-Divergenz
- EmuTOS: vr_trnfm-Device-Output + vro_cpyfm-Memory-Input beide planar → konsistent. NewTOS: vr_trnfm planar (raster.rs:454), aber vro/vrt/bb_save lesen Memory-MFDB als 32-bpp-chunky (raster.rs:115). App-Muster `Standard→vr_trnfm(→device)→vro_cpyfm(→screen)` liefert Garbage. bb_save/restore nicht betroffen (nutzen device_mem).
- 1:1-Fix: vr_trnfm-Device-Richtung muss das chunky-Format erzeugen, das vro_cpyfm erwartet. Runtime-verify: yes.

### AUD-V5-004: `vro_cpyfm` Plane-Count-Gleichheits-No-Op (`s_nxwd==d_nxwd`) fehlt
- Severity: LOW · CONFIRMED · noted deviation
- EmuTOS: `if (s_nxwd != d_nxwd) return` (vdi_raster.c:1359). NewTOS: kein Vergleich (raster.rs:266). Unter chunky-Modell benigne. Achtung: naiver Check bräche bb_save (Screen-MFDB nplanes=0) → effektive Plane-Zahl auflösen. Runtime-verify: yes.

### AUD-V5-005: `vrt_cpyfm` nur Screen-Dest; Memory-Ziel No-Op
- Severity: LOW · CONFIRMED · Gap (selten)
- EmuTOS: setup_info erlaubt Memory-d_form. NewTOS: `if …|| !dst.is_screen() { return }` (raster.rs:223). 1:1-Fix: Mono→Color-Blit auch Memory-Dest. Runtime-verify: yes.

### AUD-V5-006: OOB-Reads geguardet (sicherer als EmuTOS) — LOW/acceptable
`get_mem`→0 außerhalb Form (raster.rs:132); unbeobachtbar bei konformen Eingaben. Acceptable deviation.

### AUD-V5-007: `vr_trnfm` Early-Return bei fd_addr==0 (Screen) — LOW/acceptable
EmuTOS undefiniert bei NULL; wir no-oppen sicher (raster.rs:418). Acceptable.

### AUD-V5-008: `vrt_cpyfm` `MAP_COL[]` nicht angewandt — LOW/acceptable
MAP_COL konsistent in Palette-Reihenfolge eingefaltet (alle Primitiven PEL-identisch) → Ergebnisfarbe identisch, unbeobachtbar. Acceptable.

## Pflicht-Prüfpunkt D-092 (vro_cpyfm Save/Restore) — **GESCHLOSSEN** ✅
Kein Finding mehr. Verifiziert: vro_cpyfm im Dispatcher (mod.rs:305); screen_to_mem/mem_to_screen/mem_to_mem real (raster.rs:301-366), kein No-Op/todo!. `bb_save`/`bb_restore` (raster.rs:384) von form_alert genutzt (form.rs:333/339/342, Voll-Screen-Clip beim Restore) + Menü-Dropdown (menu.rs:460). Round-Trip byte-exakt (chunky). FB-Kohärenz beim get_device_pixel = Non-Cacheable → real-Pi-Sache, kein Logikfehler.

## Pflicht-Prüfpunkt D-082 (PAT_FLAG) — vro **korrekt**, vrt **fehlerhaft** (=V5-001)
vro_cpyfm strippt PAT_FLAG, nie Pattern (raster.rs:281) = 1:1 (Selbsttest 0x16→op6). vrt_cpyfm strippt nicht → V5-001 (schlimmer: gar keine Kopie).

## Zusammenfassung
- **Severity:** BLOCKER 0 · HIGH 0 · MEDIUM 3 (001,002,003) · LOW 5 (004,005 kleine Gaps; 006,007,008 acceptable).
- **Positiv 1:1:** 16-Op-BitBlt-Tabelle (inkl. Op-9/14-Äquivalenzen), 4 MD-Modi vrt, MFDB-ABI LP64/ILP32 inkl. Feld-Offsets, fd_addr==NULL⇒Screen, arb_corner-Normalisierung, Clip nur Screen-Dest, vr_trnfm Transpose+Rotation 1:1, D-082 auf vro, **D-092 geschlossen**. Overlap-Richtung abweichend aber korruptionsfrei (unbeobachtbar).
- **Runtime-verify offen:** V5-001..005; Selbsttest deckt vro-Ops + vrt 0-5, nicht vrt+PAT_FLAG/bb_save-Round-Trip/mem↔mem/overlap/vr_trnfm.

**Top-3:** V5-001 (vrt PAT_FLAG), V5-002 (vr_trnfm16 fehlt), V5-003 (planar vs chunky Pipeline).

**OOS:** xor_pixel invertiert nur untere 24 Bit vs raster_op(6) volle 32 (konsistent solange X=0); bb_save-FB-Kohärenz = real-Pi.
