# DESK43 U7 — G4/G5: Zellmetrik aus `graf_handle` und `vst_height` — Quittung

> Stand 2026-10-14 · Basis `main` @ `1775339d` (U5b = `ac521aeb`) · Aenderung **uncommitted** im gemeinsamen Baum
> Auftrag: `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §2.3, §3.2 (Zeile U7); Plan v2
> `docs/analysis/desktop-userspace-plan-v2.md` §2.2 B (G4/G5), §5 U7
> Messung: [`measurements/measurement-2026-10-14-desk43-u7/`](../measurements/measurement-2026-10-14-desk43-u7/RUNINFO.TXT)
> **Kein Produktverhalten geaendert (0 Pixel).** Nichts committet, keine SD-Karte, kein Pi; kein `git checkout/restore/clean/stash/reset`.
> `docs/STATUS.md` und `TASK/DESK43.md` unberuehrt.

Scope in einem Satz: Der Desktop kennt `tos_api::vdi::font` nicht mehr —
`gl_wchar`/`gl_hchar` kommen aus `graf_handle`, `gl_wschar`/`gl_hschar` aus
`vst_height(ws_chminh)` auf der Workstation, einmal je `deskmain`-Lauf gelesen.

---

## 1. Was EmuTOS tut und was hier nachgebildet ist

| EmuTOS | Fundstelle | NewTOS (U7) |
|---|---|---|
| `gl_apid = appl_init();` dann `gl_handle = graf_handle(&gl_wchar, &gl_hchar, &gl_wbox, &gl_hbox);` dann `wind_get_grect(DESKWH, WF_WXYWH, &G.g_desk)` | `desk/deskmain.c:1953-1959` | `deskmain` (`kernel/src/desk/mod.rs`): `appl_init` → **`graf_handle`** (neu, gleiche Stelle) → `wind_get_grect`; Bindung `graf_handle()` liefert jetzt `intout[0..4]` (`aes/gemsuper.c:287-293`) |
| `gl_wchar`/`gl_hchar`/`gl_wschar`/`gl_hschar` sind Globale **neben** `G` | `include/gsxdefs.h:82-86` | vier `AtomicI16` in `desk/global.rs` + `gl_wchar()`/`gl_hchar()`/`gl_wschar()`/`gl_hschar()`/`set_cell_metrics()`; atomar wie `G_PATCOL`, damit Code **innerhalb** `global::with` (z. B. `app_start`) sie ohne zweiten `DESK`-Lock liest |
| `gl_wschar`/`gl_hschar` setzt die **AES** in `gsx_start`: `gsx_textsize` (`vqt_attributes`), `vst_height(gl_ws.ws_chminh, …)`, `vst_height(gl_ws.ws_chmaxh, …)` zurueck; das ROM-Desktop liest die AES-Globale direkt (`deskapp.c:519-520,810-811,994-1002`) | `aes/gemgraf.c:404-410`, `aes/gemgsxif.c:505-512,590-600` | `vdibind::small_font_cell()`: dieselbe Folge auf derselben Workstation — `vq_extnd(h,0)` → `ws_chminh = PTSOUT[1]` (= `SIZ_TAB[1]`, `vdi/vdi_control.c:626-639`), `vqt_attributes` → aktuelle Hoehe, `vst_height(ws_chminh)` → Zelle aus `PTSOUT[2..3]`, `vst_height(aktuell)` stellt zurueck |
| Desktop zeichnet auf `gl_handle` der AES | `deskact.c:212-215` | `vdibind::set_handle()` aus demselben `graf_handle`; der zweite, faule `graf_handle` beim ersten Ziehen entfaellt |

**Abweichung vom Auftragstext (bewusst, bitte pruefen):** Auftrag und Review
§2.3 sprechen von `vst_height` „auf der **eigenen** Workstation, die
`vdibind.rs` bereits oeffnet". Das stimmt seit K1 (2026-10-07) nicht mehr:
`vdibind.rs` oeffnet **keine** eigene Workstation, der Desktop teilt die
physische Workstation der AES wie das EmuTOS-Desktop (`vdibind.rs`-Modulkopf).
Ich habe deshalb **keine** neue Workstation eroeffnet (das haette K1
rueckgaengig gemacht und `v_opnvwk`/`v_clsvwk` hinzugefuegt), sondern die
`gsx_start`-Folge der AES selbst wiederholt, einschliesslich Rueckstellung der
Texthoehe — die gemeinsame Workstation endet im Ausgangszustand
(`vst_height` exakt auf eine Ringgroesse setzt nur `cur_font`/`scaled=false`,
`tos-api/src/vdi/attributes.rs:263-296`). Belegt durch Messung: Werte 8/16/6/6
(§3), Oracle-Diff (§3), 0 Pixel (§3).

## 2. Was sich geaendert hat

| Datei | Aenderung |
|---|---|
| `desk/global.rs` | `GL_WCHAR/GL_HCHAR/GL_WSCHAR/GL_HSCHAR` (`AtomicI16`), Leser `gl_*()`, `set_cell_metrics()` |
| `desk/mod.rs` | `graf_handle()` → `(gl_handle, gl_wchar, gl_hchar, gl_wbox, gl_hbox)`; `deskmain` fragt nach `appl_init`, setzt Handle + Metrik **vor** `global::init` |
| `desk/vdibind.rs` | `OP_VST_HEIGHT/OP_VQT_ATTRIBUTES/OP_VQ_EXTND`, `set_handle()`, `small_font_cell()`; `handle()` liest nur noch (kein fauler `graf_handle`); Modulkopf ergaenzt |
| `desk/icons.rs` | `IB_WTEXT`/`IB_HTEXT` `const` → `ib_wtext()`/`ib_htext()` (Plan v2 U7 sagt genau das voraus) |
| `desk/menu.rs` | `CHAR_W` (= `gl_wchar`) geloescht; `BAR_BOX_H` → `bar_box_h()`, `ENTRY_H` → `entry_h()` (Doku unveraendert, zitiert `desk_rsc.c`/`gemrslib.c`) |
| `desk/dir.rs`, `desk/inf.rs` | lokale `const` aus Zellmetrik → `let` (`btn_w/btn_gap/btn_row_w`; `row_pitch`; `row_h/def_w/lbl_w/fmtgrp_w/group_w`) |
| `desk/{act,app,ins,win}.rs` | reine Ersetzung |

**Stellen:** 261 Code-Nutzungen auf 151 Zeilen (`CELL_W` 128, `CELL_H` 128,
`SMALL_CELL_W` 2, `SMALL_CELL_H` 3) + 8 Importzeilen + 4 Doku-Erwaehnungen; dazu
die abgeleiteten `CHAR_W` (9), `ENTRY_H` (4), `BAR_BOX_H` (2), `IB_WTEXT` (5),
`IB_HTEXT` (2). (Review „270 + 9" zaehlte Doku/Importe mit.) Kein neues `unsafe`,
kein `kprintln`, keine neue Abhaengigkeit.

## 3. Pruefungen (Einzelheiten RUNINFO)

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1347/0/1 | **1347/0/1** |
| Release-Warnungen rpi4 / qemu-virt | 64/94 | **64/94** beide, 0 Fehler |
| Clippy rpi4 / qemu-virt | 76/125 | **76/125** beide; Verteilung je Datei identisch zu U5b, 0 in `desk/` |
| Driftprobe / unsafe | 11 / ≤ 24 | **11** (A 0 · B 7 · C 4 · D 0) / **24** |
| `cargo xtask emutos-consts` | gruen | total=448 matched=281 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=165 (+3 = die drei neuen `OP_*`-Opcodes) |
| SDK-Hosttests | 20/0/0 | **20/0/0** |
| `programs/build.sh` | baut | baut; HELLOGEM.NAP byte-gleich zu U5b (`3110e2e3…`) |
| QEMU raspi4b on/off/strict: Desktop, APITEST, TeraDesk, QED (12 Laeufe) | = U5b | Screendumps **15/15 byte-gleich** zu U5b `u5b-*` **und** `base-*`; APITEST **106/0/5** alle Modi, 0 Zeilen Diff; 0 `[EXC]`/`PANIC` |
| Laufwerksfenster (Alt-C), Textansicht, Iconansicht, About, Voreinstellungen, Hintergrund, Desktop-Konfiguration | = HEAD | neuer Injektor (Tasten + IKBD-Maus), HEAD vs U7: **8/8 byte-gleich**; Werte im Lauf: `gl_handle=1 gl_wchar=8 gl_hchar=16 gl_wschar=6 gl_hschar=6` |
| qemu-virt Boot | Desktop | `desktop up`, End-Screendump byte-gleich zu U5b-Kern |
| Oracle (G2) | nur neue Init-Aufrufe | s. u. |

**Oracle-Diff (exakt):** `SVCSEQ-base.LOG` (HEAD) = U5b-Log byte-gleich (684
Zeilen/551 Aufrufe; der HEAD-Seq-Kern ist sogar sha-gleich zum U5b-Seq-Kern).
U7: 690 Zeilen/556 Aufrufe. Zwischen `appl_init -> 0` und `wind_get(DESKWH)`
neu: `AES 77 graf_handle -> 1 (handle=1 wchar=8 hchar=16 wbox=19 hbox=19)`,
`VDI 102`, `VDI 38`, `VDI 12`, `VDI 12`. Sonst nur der **Rueckgabewert** von
`Keytbl` (XBIOS 16): `…349d08` → `…34ad08` = `tos_api::xbios::keytbl::KERNEL_AREA`
in `.bss`, eine Seite weiter, weil `.text` 866 464 → 871 988 B wuchs (Ladebefehle
statt Konstanten). Die Seq-Screendumps unterscheiden sich in 62 Pixeln der
Info-Zeile („3423082" vs „3423301 bytes used"): +219 B = genau die sechs neuen
Zeilen der Spurdatei `C:\SVCSEQ.LOG`, die selbst im Fenster liegt; ohne Tracer
(Ansichtslauf oben) ist dasselbe Fenster byte-gleich.

## 4. Guards (`docs/api/desktop/_rebuild-plan.md` §12)

```
Guards (docs/api/desktop/_rebuild-plan.md §12):
  DRIFT-PROBE TOTAL: 11 (before: 19)   DRIFT-PROBE UNSAFE: 24 (baseline 24)
  EMUTOS-CONSTS: total=448 matched=281 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=165
                 (EmuTOS snapshot 1a7484ab, unchanged; +3 no-counterpart = vdibind OP_VST_HEIGHT/OP_VQT_ATTRIBUTES/OP_VQ_EXTND)
  DESK-CITE: fns=541 cited=357 newtos=0 uncited=184            (whole desktop; before 530/346/0/184)
  DESK-CITE --since 1775339d: 39 fns touched, 34 cited, 5 uncited -- all 5 touched only by CELL_* -> gl_*() replacement or by
                 an insertion below them; no logic touched:
                 act.rs rect_hits_obj (LEN_FNODE * gl_wchar()), dir.rs CpyDelBox::new (fld_w/title_w/y_folds),
                 global.rs patcol_all (new cell-metric block inserted after it, attributed to it),
                 inf.rs Line::width (len * gl_wchar()), menu.rs build_menu (bar_box_h()/entry_h()/gl_wchar()).
                 All 11 new fns cite EmuTOS (gl_wchar/gl_hchar/gl_wschar/gl_hschar/set_cell_metrics: gsxdefs.h/deskmain.c/
                 gemgraf.c; ib_wtext/ib_htext: deskapp.c:519-520; bar_box_h/entry_h: desk_rsc.c/gemrslib.c;
                 set_handle/small_font_cell: deskmain.c:1956, gemgraf.c:404-410). No // NEWTOS: fn added.
  Oracle: run before/after (required: the unit changes what the desktop calls): diff = +graf_handle, +vq_extnd,
          +vqt_attributes, +2x vst_height at init, Keytbl pointer value moved one page; measurements/measurement-2026-10-14-desk43-u7/checks/u7-oracle-diff.txt
```

## 5. Offen fuer U8a (und spaeter)

1. **`MENU_H = 19` ist `gl_hbox`** (`desk/global.rs:35`, Doku „NewTOS uses its
   fixed 8x16 font"), handgeschrieben; `graf_handle` liefert `gl_hbox` jetzt
   bereits in `deskmain` (`_gl_hbox`, ungenutzt). Gleiches Muster wie U7, drei
   Nutzungen in `menu.rs`. Nicht gemacht (Auftrag: `CELL_*`).
2. **VDI-Konstanten nicht konsolidiert** (U5b-Punkt 1): fiel **nicht** natuerlich
   ab — U7 beruehrt keine Farb-/`FIS_*`/`MD_*`-Konstante. Stand unveraendert:
   `WHITE/BLACK` als `Pel` in `tos-api/src/aes/{object,graf,form,window,menu}.rs`,
   als `i16` in `kernel/src/initinfo.rs`, `desk/vdibind.rs` (`BLACK`, `MD_XOR`),
   `newtos-gem/src/vdi.rs`; `FIS_*`, `MD_REPLACE`, `MAX_*` mehrfach. Typfrage
   `Pel` vs `i16` vor einem `tos_types::vdi` klaeren.
3. **VDI-Opcodes im Desktop** heissen `OP_*` (jetzt 8 in `vdibind.rs`), EmuTOS
   nennt sie `SET_CHAR_HEIGHT`/`INQ_TEXT_ATTRIBUTES`/… (`gsxdefs`); als
   EmuTOS-Namen waeren sie fuer G3 pruefbar statt „no-counterpart". Sinnvoll mit
   U6b, wenn die Bindungen nach `newtos-gem` ziehen.
4. **Selbsttests ohne `deskmain`** (DESK45): `inf/ins/dir`-`selftest()` haben
   keinen Aufrufer; wer sie verdrahtet, muss sie nach `deskmain` laufen lassen
   oder vorher `set_cell_metrics` setzen — sonst sind alle Metriken 0.
5. Die vier Metrik-Globalen sind `AtomicI16` (kein Lock) — unabhaengig von der
   `SpinMutex`→`RefCell`-Umstellung in U8a, funktionieren in EL0 unveraendert.
6. Review §2.3 / Plan v2 U7 (`vdibind.rs` „oeffnet eine eigene Workstation")
   ist seit K1 veraltet → Dokumentkorrektur (nicht meine Datei).
7. Nicht gefahren: Install-Icon/-Application-Dialoge (`ins.rs`, 21 Stellen) und
   Kopier-/Umbenennen-Dialoge (`dir.rs`) — braeuchten Auswahl + Dateioperation
   auf der Karte. Die Umstellung dort ist dieselbe mechanische Ersetzung.

## 6. Ausserhalb des Auftrags (nicht angefasst)

* EmuTOS nutzt `gl_wchar`/`gl_hchar` auch in `deskobj.c:70`, `deskwin.c:177-178`,
  `desksupp.c:798`, `deskmain.c:1699,1737-1744`, `deskapp.c:898-904,1200-1202`;
  ob NewTOS dort ueberall `gl_*()` oder ein Zahlenliteral nutzt, ist **nicht**
  geprueft (nur die `CELL_*`-Importe waren Auftrag). `desksupp.c:267` nutzt `gl_wbox`.
* Kernel-Abbilder sind zwischen Baumpfaden nicht byte-reproduzierbar
  (HEAD aus `git archive` ≠ U5b-`kernel8-u5b.img` trotz gleichem Quelltext und
  `SOURCE_DATE_EPOCH`); die Seq-Kerne aus gleich strukturierten Scratch-Baeumen
  sind es. Fuer Byte-Vergleiche nur Kerne aus gleich angelegten Baeumen nehmen.
* `hdiutil attach -readonly -nomount` meldet sich als veraltet (wie U5a/U5b).

## 7. Selbstpruefung

Aufgabe in einem Satz: `CELL_*`/`SMALL_CELL_*` im Desktop durch die zur Laufzeit
von `graf_handle`/`vst_height` gelesenen Werte ersetzen, ohne Pixelaenderung.
Geaendert: 11 Dateien `kernel/src/desk/` (`act app dir global icons inf ins menu
mod vdibind win`), diese Quittung, das Messverzeichnis. Nicht angefasst:
`tos-api`, `tos-types`, `programs/`, `docs/STATUS.md`, `TASK/DESK43.md`,
`emutos-consts-deviations.txt`, `UNSAFE_BASELINE`. Beide Boards bauen
(64/94, 76/125).
