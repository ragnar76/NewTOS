# DESK43 U5b — Typen + Konstanten nach `tos-types` — Quittung

> Stand 2026-10-14 · Basis `main` @ `f6f14885` · Aenderung **uncommitted** im gemeinsamen Baum
> Auftrag: `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.2 (Zeile U5b), §2.1, §4 (G3);
> offene Frage 3 aus `TASK/DESK43-U6a-STATUS-DONE.MD` (Heimat von `Grect`/`WF_*`/`WM_*`)
> Messung: [`measurements/measurement-2026-10-14-desk43-u5b/`](../measurements/measurement-2026-10-14-desk43-u5b/RUNINFO.TXT)
> **Reiner Umbau, kein Produktverhalten.** Nichts committet, keine SD-Karte, kein Pi; kein `git checkout/restore/clean/stash/reset`.
> `docs/STATUS.md` und `TASK/DESK43.md` unberuehrt. `BLKDEVNUM` nicht angefasst (TASK/DESK58).

Scope in einem Satz: `Object`/`TedInfo`/`IconBlk` (+ `Grect`) und **ein**
oeffentlicher Satz der AES-/Objekt-Konstanten wohnen in `tos-types`
(`tos_types::aes`); jede private Kopie in AES, Kernel-Selbsttests, Desktop und
`programs/newtos-gem` ist geloescht; Layout per `const`-Assertion festgenagelt.

---

## 1. Was wohin gewandert ist

### 1.1 Neue Heimat `tos-types/src/aes.rs` (Modul `tos_types::aes`)

| Inhalt | Herkunft (geloescht dort) |
|---|---|
| `Object`, `TedInfo`, `IconBlk` (LP64-Records, Felder/Derives unveraendert) | `tos-api/src/aes/object.rs` (dort jetzt `pub use tos_types::aes::{IconBlk, Object, TedInfo}` — alle bisherigen Pfade `tos_api::aes::object::X` gelten weiter) |
| `Grect` (`x/y/w/h: i16`, Derives `Clone, Copy, Debug, Default, PartialEq, Eq`, `intersect`) | **drei** Kopien: `tos-api/src/aes/window.rs` (privat), `kernel/src/desk/global.rs`, `programs/newtos-gem/src/aes.rs` |
| 134 Konstanten, jede mit EmuTOS-`datei:zeile`: `ROOT NIL`, `IP_*` (3), `G_*` (14), `ob_flags` (15 inkl. `FL3D*`, `SUBMENU`), `ob_state` (7), `objc_sysvar`/3D (8), `ED_*` (4), `TE_*` (3), Nachrichten `MN_SELECTED WM_* AC_*` (13), `WA_*` (8), `MU_*` (7), `FMD_*` (4), `graf_mouse` (8), Fensterteile (13), `WF_*` (18), `WC_* END_/BEG_UPDATE END_/BEG_MCTRL` (6), `DESKWH` | 377 private Definitionen in 25 Dateien (Tabelle 1.2) |

Breiten folgen dem Feld, in das der Wert geht (vorher schon so, jetzt
dokumentiert): `G_*`/`ob_flags`/`ob_state`/Fensterteile/`MU_*`/`IP_*` `u16`,
alles, was in `intin[]` als `WORD` geht, `i16`. Benannt nach EmuTOS
(`WF_WXYWH`/`WF_CXYWH`, nicht gemlib `WF_WORKXYWH`/`WF_CURRXYWH`); einzige
Ausnahme `ED_START/ED_INIT/ED_CHAR/ED_END` (EmuTOS `EDSTART…`, Name aus dem
Baum behalten, Zitat steht dabei).

`tos-types` hat keine Abhaengigkeiten und baut als Pfad-Abhaengigkeit im
eigenen `programs/`-Workspace fuer `aarch64-unknown-none-softfloat` mit
`-C relocation-model=pic` ohne jede Aenderung (`newtos-gem/Cargo.toml`:
`tos-types = { path = "../../tos-types" }`; `programs/Cargo.lock` traegt den
neuen Eintrag). `newtos-gem::aes` re-exportiert die von ihm vorher selbst
deklarierten 24 Namen + `Grect` (`pub use`), Programme schreiben weiter
`aes::WM_REDRAW`; `WF_WORKXYWH`/`WF_CURRXYWH` heissen jetzt wie in EmuTOS
`WF_WXYWH`/`WF_CXYWH` (`hello-gem` angepasst, gleiche Werte 4/5).

**Entscheidung offene Frage 3 (U6a):** GEM-Records, `Grect` und alle
AES-Konstanten wohnen in `tos_types::aes` — fuer Kernel, AES, Desktop und
EL0-Programme. U6b kopiert nichts mehr.

### 1.2 Geloeschte private Kopien

| Crate | Definitionen geloescht | davon |
|---|---:|---|
| `tos-api/src/aes/` (12 Dateien) | 206 | window 63, object 50, form 25, menu 13 (+2 umbenannte Kopien `G_BOX_T`/`G_IBOX_T`), fsel 12 (+`IP_4PATT: u64`), resource 10, event 9, graf 8, init 4 (+`ROOT: usize`), pd 4, input 3, trace 1 |
| `kernel/src/desk/` (10 Dateien) | 120 | mod 27, inf 23, ins 14, main 10, win 10, dir 9, dlg 9, insapp 9, global 8, act 1 (`OB_DISABLED`) |
| `kernel/src/` Selbsttests/Debug | 27 | `selftest_objc` 24, `dbg_qedtrace` 3 |
| `programs/newtos-gem` | 24 | `aes.rs` (inkl. `NAME/CLOSER/MOVER` als `i16`) |
| **Summe** | **377** | 134 Namen + 4 Aliasnamen (`OB_SELECTED`, `OB_DISABLED`, `WF_WORKXYWH`, `WF_CURRXYWH` → kanonisch umbenannt) |

`G_STRING = 28`: **11 → 1**. `LASTOB` 11 → 1, `G_BOX` 10 → 1 (+`G_BOX_T`),
`DEFAULT` 10 → 1 (der `vdi::font::DEFAULT` gleichen Namens ist etwas anderes
und bleibt). Kopien mit anderem Typ (`ROOT: usize` in `aes/init.rs`,
`IP_4PATT: u64` in `aes/fsel.rs`, `NAME/CLOSER/MOVER: i16` in `newtos-gem`)
sind ebenfalls weg; die Verwender casten an der Stelle (`ROOT as usize`,
`u64::from(IP_4PATT)`, `(NAME | CLOSER | MOVER) as i16` in `hello-gem`).

Die per-Konstante-Dokumentation, die Inhalt trug (z. B. `G_IBOX` als
Radiogruppen-Eltern, `TOUCHEXIT`, `OUTLINED`-3D-Rahmen, `FL3D*`, `HOTCLOSE`,
`DESKWH`, `AC_CLOSE`), ist in `tos_types::aes` uebernommen; verwaiste
Abschnittskommentare zeigen jetzt auf `tos_types::aes`.

### 1.3 Mechanik in `tos-api/src/aes/object.rs`

`impl TedInfo { unsafe fn read_abi }` / `impl IconBlk { … }` sind auf einem
fremden Typ nicht mehr erlaubt → freie Funktionen `tedinfo_read_abi` /
`iconblk_read_abi`, Rumpf zeichengleich, Aufrufer (5) umbenannt.

### 1.4 G3-Werkzeug mitgezogen

`tools/xtask/src/emutos_consts.rs`: neues `SHARED_SETS = ["tos-types/src/aes.rs"]`,
gescannt zusammen mit dem Desktop. Ohne das waeren die 120 aus `desk/`
geloeschten Konstanten aus der EmuTOS-Pruefung gefallen. Die Datei der
Abweichungen ist **unveraendert** (DEOK, BLKDEVNUM bleiben im Desktop).

## 2. Pruefungen (`checks/u5b-*`, frisch, beide Boards, `SOURCE_DATE_EPOCH` fest)

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1345/0/1 + neue | **1347/0/1** (+2: `aes::tests::native_record_layout`, `aes::tests::grect_intersect`) |
| Layout-Assertions | neu | `const _: () = { … }` in `tos-types/src/aes.rs`: Groesse/Ausrichtung und **jeder** Feld-Offset von `Object` (32 B, `ob_spec` @16, `ob_x` @24), `TedInfo` (40 B, `te_font` @24), `IconBlk` (48 B, `ib_char` @24), `Grect` (8 B) — Compilezeit, jedes Ziel |
| Release-Warnungen rpi4 / qemu-virt | 64/94 oder weniger | **64/94** beide, 0 Fehler |
| Clippy rpi4 / qemu-virt | 76/125 oder weniger | **76/125** beide; `tos-types` 0 |
| Driftprobe / unsafe | 19 erwartet / ≤ 24 | **19** (A 8 · B 7 · C 4 · D 0) / **24** |
| `cargo xtask emutos-consts` | gruen | total=445 matched=281 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=162 |
| SDK-Hosttests | 20/0/0 | **20/0/0** |
| `programs/build.sh` (HELLOGEM.NAP) | baut | baut, 0 Warnungen; Clippy `programs/` (Standard + `panic-test`) **0** |
| Clippy `tos-types --all-targets` / `xtask` | — | 0 / 1 (vorbestehend `xtask/src/main.rs:285`; 0 in `emutos_consts.rs`) |
| Merkmal-Builds | — | `cargo check` Kernel beide Boards mit `aes-trace,gemdos-trace,svc-seq-trace,selftest-emmc,usb-diag,initinfo-diag`, `tos-api --tests` mit/ohne Tracer, gefiltert auf unused/never used/unreachable/Bindungs-Warnungen: nur die vorbestehenden (`CYCLES_AFTER_KEY`, `boot_l0_phys`; Tests: `Call`, `MIB2`) |

Driftprobe 31 → 19: die 12 Zeilen `use tos_api::aes::object::…` in `desk/`
(Gruppe A) sind weg; uebrig in A die 8 `tos_api::vdi::font`-Zeilen (`CELL_*`,
U7). `unsafe` unveraendert 24 (`UNSAFE_BASELINE` bleibt).

**Gefahr, die der Umbau hatte, und wie sie ausgeschlossen ist:** Eine
geloeschte Konstante, die in einem `match`-Arm steht und nicht importiert
wird, kompiliert weiter — als Fang-alles-Bindung. Jede Bauvariante wurde
deshalb zusaetzlich auf `unused variable`/`non_snake_case`/`unreachable
pattern` fuer Namen des Satzes geprueft (so wurden 16 Namen `WF_*`/`*_UPDATE`/`*_MCTRL`
in `match`-Armen von `aes/window.rs` gefunden und importiert); Endstand: **keine** solche
Warnung in irgendeiner Variante. Der Maschinencode-Vergleich (§3) bestaetigt
es unabhaengig.

## 3. Kein Verhalten geaendert — Beweis (Einzelheiten RUNINFO §2, §4, §5)

1. **Maschinencode** (HEAD vs U5b, beide als Scratch-Kopie gebaut, gleiche
   Datumsmarke, je Funktion verglichen): rpi4 992/992 Funktionen, **985
   identisch**; `TedInfo::read_abi` → `tedinfo_read_abi` identisch bis auf den
   Namen, seine 4 Aufrufer unterscheiden sich nur im Namen des Aufgerufenen;
   **2 Funktionen anders uebersetzt**: `aes/window.rs` `wind_open` und
   `draw_change_open` — Quelltext unveraendert, aber das `Grect` des
   Fenstermanagers ist jetzt der `tos-types`-Typ (gleiches Layout); der
   Optimierer kopiert es dort als vier Halbworte statt als ein 64-Bit-Wort
   (Gegenprobe im Scratch: mit dem alten Derive-Satz dieselben zwei
   Funktionen, `checks/u5b-exp-grect-derives-rpi4.txt`). `.data` identisch;
   `.rodata` gleich gross, nur 198 Panic-Zeilennummern (alle in editierten
   Dateien) + 76 Codezeiger um genau 0x14 verschoben (die 20 Byte kuerzere
   `.text`). qemu-virt: dasselbe Bild (956 Funktionen).
2. **HELLOGEM.NAP**: sha256 `84cc2a08…` → `3110e2e3…`, **nicht** sha-gleich,
   aber alle 72 Funktionen / 5 423 Befehle identisch, jede geladene Sektion
   gleich gross; Unterschied = Platzierung der Funktionen und `.strtab`
   (−2 Byte), weil die verstuemmelten Namen den neuen Crate-Hash von
   `newtos-gem` tragen (es haengt jetzt an `tos-types`).
3. **Bildschirm + APITEST**, QEMU raspi4b, memprot on/off/strict, alter und
   neuer Kern auf denselben Kartenkopien: siehe §3a unten.
4. **Aufrufsequenz-Orakel (G2)**, alter und neuer Kern mit demselben
   Injektor (Alt-C 1 s nach „desktop up") auf derselben Kartenkopie:
   `SVCSEQ.LOG` **byte-gleich** (684 Zeilen, 551 Aufrufe, einschliesslich
   Ergebnissen) — und byte-gleich zum U5a-Lauf. Screendumps t20/t45
   (Laufwerksfenster C:\ offen) identisch zueinander und zu U5a.

### 3a. QEMU raspi4b (Endstand-Kern `bin/kernel8-u5b.img`, `checks/u5b-series-summary.txt`)

Alter Kern (`kernel8-base.img`, HEAD) und neuer Kern, je memprot on / off /
strict, dieselben Kartenkopien (MEM05-Abnahme): Desktop, APITEST, TeraDesk,
QED — 24 Laeufe, 0 `[EXC]`/`PANIC`.

| | on | off | strict |
|---|---|---|---|
| Screendumps u5b = base (Desktop t25, APITEST t25/t40, TeraDesk t35, QED t30) | 5/5 | 5/5 | 5/5 |
| Screendumps u5b = MEM05-Abnahme `acc-<mode>-rpi4-*` | 5/5 | 5/5 | 5/5 |
| Desktop t25 sha256 | `0e6f7704…324b` (= U5a/U6a/MEM05) | gleich | gleich |
| APITEST TOTAL / Zeilen-Diff zu base / zu MEM05 | 106/0/5 · 0 · 0 | 106/0/5 · 0 · 0 | 106/0/5 · 0 · 0 |

Ein frueherer U5b-Kern (vor dem letzten Aufraeumen der `use`-Zeilen) lief
dieselbe Serie mit demselben Ergebnis (`prelim/`, nicht versteckt).

## 4. Guards (`docs/api/desktop/_rebuild-plan.md` §12)

```
Guards (docs/api/desktop/_rebuild-plan.md §12):
  DRIFT-PROBE TOTAL: 19 (before: 31)   DRIFT-PROBE UNSAFE: 24 (baseline 24)
  EMUTOS-CONSTS: total=445 matched=281 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=162
                 (EmuTOS snapshot 1a7484ab, unchanged; scan now desktop + tos-types/src/aes.rs; desktop alone: 311/151/2/2/0/0/158)
  DESK-CITE: fns=530 cited=346 newtos=0 uncited=184            (whole desktop, unchanged)
  DESK-CITE --since f6f14885: 47 fns touched, 32 cited, 15 uncited -- all 15 touched only by the rename of a
                 constant (OB_SELECTED -> SELECTED, OB_DISABLED -> DISABLED, win::DESKWH -> DESKWH, super::MU_* -> MU_*)
                 or by deleted constants/imports/doc-link edits between fns, attributed to the fn above them; no logic touched:
                 act.rs is_selected, dir.rs select_all, global.rs default + new, inf.rs set_background,
                 ins.rs selftest, insapp.rs set_flavor_radio + set_param_radio, main.rs selected_native_program,
                 mod.rs objc_find + wind_find + wind_get_top, win.rs build_item + clear_obj_selection + count_selected.
                 No // NEWTOS: fn added.
  Oracle: run before/after (not required for U5b, done as proof): SVCSEQ.LOG byte-identical, 551 calls,
          measurements/measurement-2026-10-14-desk43-u5b/checks/u5b-oracle-diff.txt
```

Beruehrte `fn` ausserhalb von `desk/` ohne EmuTOS-Zitat: `tos_types::aes::Grect::intersect`
(aus `newtos-gem` verschoben, zitiert `rc_intersect`), `tos-api` `tedinfo_read_abi`/`iconblk_read_abi`
(umbenannt, Zitat `mt_gem.h` unveraendert), `xtask::emutos_consts::desktop_consts` (Host-Werkzeug).

## 5. Offen fuer U7 / U6b

1. **VDI-Konstanten nicht konsolidiert** (U7): Farben `WHITE/BLACK/…` stehen
   als `Pel` in `tos-api/src/aes/{object,graf,form,window,menu}.rs`, als `i16`
   in `kernel/src/initinfo.rs`, `desk/vdibind.rs`, `newtos-gem/src/vdi.rs`;
   `FIS_*`, `MD_REPLACE`, `MAX_*` der VDI ebenso mehrfach. Gleiches Muster
   anwendbar (`tos_types::vdi`), aber Typfrage `Pel` vs `i16` zuerst klaeren.
2. **AES-Opcodes doppelt, unter zwei Namen**: `desk/mod.rs` `OP_*` (36) und
   `newtos-gem` `APPL_INIT…` (14). Sinnvoll mit U6b, wenn `newtos-gem` die
   Desktop-Bindungen uebernimmt (dann ein Satz, z. B. in `tos_types::aes`).
3. **GEMDOS-Opcodes/Fehlercodes doppelt**: `DSETDRV/DSETPATH/FDELETE` in
   `aes/shell.rs` und `desk/dosbind.rs`; `newtos-gem/src/gemdos.rs`
   `ENSMEM/ERROR` neben `tos_types::error`. Nicht Teil von U5b.
4. `BitBlk`, `UserBlk`, `ParmBlk`, `Cicon` bleiben in `tos-api` (der Desktop
   nutzt sie nicht); fuer EL0-Rust-Programme mit Bildern spaeter nach
   `tos_types::aes`.
5. `newtos-gem::aes::wind_create/wind_calc` nehmen `kind: i16`, die
   Fensterteile sind jetzt `u16` → `hello-gem` castet. U6b: API auf `u16`
   (wie `WINDOW_STYLE` im Desktop) entscheiden.
6. `MAX_DEPTH` bleibt lokal (8 in `window/shell/desk`, 16 in
   `fsel/form/menu` — verschiedene Bedeutung, kein Duplikat); `NONE`
   (Fenster-`kind` 0) und die Fenster-Glyphen `G_UP…G_FULL` bleiben in
   `aes/window.rs` (keine API-Konstanten).
7. Die ~340 Konstanten in `tos-api/tests/*.rs` sind bewusst **nicht**
   umgestellt: Hosttests mit eigenem Literal sind eine unabhaengige Gegenprobe
   der Werte im Satz.

## 6. Ausserhalb des Auftrags (nicht angefasst)

* `docs/api/structures/object.md:47-50` beschreibt das native `OBJECT` mit
  `ob_spec` bei 0x0C und `ob_x` bei 0x14; tatsaechlich (und jetzt per
  Assertion festgeschrieben, wie `ObjTree`/`OB_SPEC_OFF_LP64 = 16` es lesen)
  0x10 und 0x18. Spec-Korrektur → Researcher.
* `kernel/src/desk/act.rs` `act_chg` prueft `ob_flags & DISABLED`; EmuTOS
  `deskact.c:429` prueft den **Zustand** (`curr_state & DISABLED`). Wert und
  Verhalten unveraendert uebernommen (vorher `OB_DISABLED` auf `ob_flags`).
* `tos-api/src/aes/window.rs` `ob_type: 20, // G_BOX` — Literal statt Name (kein Konstanten-Duplikat, nicht geaendert).
* Der Baum ist nicht `rustfmt`-sauber (schon vorher); nur die neuen
  `use tos_types::aes::…`-Zeilen sind `rustfmt`-formatiert.
* `hdiutil attach -readonly -nomount` meldet sich als veraltet (wie U5a).

## 7. Selbstpruefung

Aufgabe in einem Satz: GEM-Records + ein oeffentlicher AES-Konstantensatz in
`tos-types`, alle privaten Kopien geloescht, ohne Verhaltensaenderung.
Geaendert: `tos-types/src/{aes.rs (neu), lib.rs}`, 12 Dateien
`tos-api/src/aes/`, 20 Dateien `kernel/src/` (17 `desk/`, `selftest_objc`,
`selftest_aes`, `dbg_qedtrace`), `programs/{newtos-gem/Cargo.toml,
newtos-gem/src/aes.rs, hello-gem/src/lib.rs, Cargo.lock}`,
`tools/xtask/src/emutos_consts.rs`, diese Quittung, das Messverzeichnis.
Nicht angefasst: `BLKDEVNUM`, `emutos-consts-deviations.txt`,
`docs/STATUS.md`, `TASK/DESK43.md`. Beide Boards bauen (64/94, 76/125).
