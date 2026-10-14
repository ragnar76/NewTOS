# DESK43 U5a — Instrument vor Umzug (Guards) — Quittung

> Stand 2026-10-14 · Basis `main` @ `401b1bc0` · Aenderung **uncommitted** im gemeinsamen Baum
> Auftrag: `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.2 (Zeile U5a), §4 (G2-Vorarbeit, G3, G4, G5), §2.4 (Leer-Null-Falle)
> Messung: [`measurements/measurement-2026-10-14-desk43-u5a/`](../measurements/measurement-2026-10-14-desk43-u5a/RUNINFO.TXT)
> **Kein Produktverhalten.** Nichts committet, keine SD-Karte, kein Pi; kein `git checkout/restore/clean/stash/reset`.
> `docs/STATUS.md` und `TASK/DESK43.md` unberuehrt.

Scope in einem Satz: vier Messinstrumente bauen, die vor dem Umzug des
Desktops Drift sichtbar machen — Probe v2 (G5), Konstantenabgleich gegen
EmuTOS (G3), Zitat-Liste (G4), Aufrufsequenz-Orakel (G2-Vorarbeit) — ohne
das Produkt zu aendern.

---

## 1. Die vier Instrumente

### 1.1 G5 — `tools/desk-drift-probe.sh` v2

| Blindstelle v1 (Review) | v2 |
|---|---|
| `kernel/src/desk/` fehlt → `TOTAL: 0`, Exit 0 (§2.4, `:74-80`) | sucht dann die Crate `desktop` (`Cargo.toml` mit `name = "desktop"`); findet es keines von beiden: **Exit 1 mit Meldung**, keine Null. `DESK_SRC=<dir>` als Uebersteuerung |
| nur `$DESK_DIR/*.rs` (§2.4, `:131`) | rekursiv, Unterverzeichnisse eingeschlossen |
| Klammer-`use` (`crate::{…}`) nicht gezaehlt (§4.3) | mehrzeilige `use`-Anweisungen werden zusammengefuegt (eine Zeile = ihre erste), Klammerlisten pro Pfad aufgeloest |
| Gruppe C matcht nach Umbenennung `desktop::` nicht (§2.4) | C zaehlt `desk`-Modul **und** `desktop`-Crate (`desktop::…`, `extern crate desktop`, Abhaengigkeitszeile in `kernel/Cargo.toml`) |
| keine `Cargo.toml`-Pruefung (G1/G5) | neue Gruppe **D**: Abhaengigkeiten der Desktop-Crate ausser `newtos-gem`/`tos-types`; heute 0 („no desktop crate yet") |
| `unsafe` nicht beobachtet (G5) | `unsafe`-Codezeilen als eigene Zahl, **nicht** in TOTAL; Basislinie `UNSAFE_BASELINE=24` im Skript; steigt sie: Fehlermeldung + **Exit 3** |
| Kommentar-Abschneiden ab `//` | dazu `/* … */` auch mehrzeilig; String-/Char-Literale werden erkannt (`b"c:/*.txt"` in `selftest_emmc.rs` haette sonst den Rest der Datei verschluckt) |
| B zaehlt `crate::` — als eigene Crate waere das der Desktop selbst | im Crate-Modus zaehlt B `kernel::` |

Ausgabeformat kompatibel: Abschnitte A/B/C wie v1 (Beschriftung im
Modul-Modus byteweise gleich), letzte Zeile `DRIFT-PROBE TOTAL: <n>`, `-q`
unveraendert; neu `-u` (`DRIFT-PROBE UNSAFE: <n>`).

**Heute:** TOTAL **31** (A 20 · B 7 · C 4 · D 0), UNSAFE **24**. Die
gezaehlten Zeilen sind identisch mit v1 (`checks/u5a-drift-v1-verbose-before.txt`
gegen `checks/u5a-drift-v2-verbose.txt`: nur Kopfzeile, D-Zeile und
`unsafe`-Block neu).

**Belegt am Gegenbeispiel** (`method/probe-fixture.sh`, nur Scratch-Kopien):
Fall 1 `desk/` weg, keine Crate → Exit 1, Meldung. Fall 2 `desk/` als Crate
`desktop/` mit Unterverzeichnis, Klammer- und mehrzeiligem `use`,
Blockkommentar, `"c:/*.txt"`, verbotenen Abhaengigkeiten (`tos-api`, `spin`),
Kernel ruft `desktop::` + `Cargo.toml`-Abhaengigkeit → A 22 · B 1 · C 4 · D 2,
UNSAFE 25 → Exit 3. Gruppe C = genau die vier Zeilen aus Review §3.2 Fussnote 1.

### 1.2 G3 — `cargo xtask emutos-consts`

**Entscheidung (Review §4.4): Snapshot + Hosttest**, nicht lokales Werkzeug.
`references/emutos/` ist gitignored; ein Test, der es liest, liefe weder im
CI noch auf einem frischen Klon. Deshalb:

* `tools/xtask/emutos-defines.snapshot` — **1546** ganzzahlige `#define`s mit
  `datei:zeile` aus `references/emutos @ 1a7484ab`, Dateimenge
  `include/*.h desk/*.h aes/*.h desk/*.c` (wie der Review-Prototyp). Erzeugt
  nur mit `cargo xtask emutos-consts --update` (bricht ohne lokale
  EmuTOS-Kopie mit Meldung ab), Kopf nennt den EmuTOS-Commit.
* `tools/xtask/emutos-consts-deviations.txt` — jede Abweichung mit Art
  `intended` (Entscheidung) oder `defect` (bekannt falsch) und Begruendung.
* Hosttest `emutos_consts::tests::desk_consts_match_emutos_snapshot` (laeuft
  in `cargo xtask test`/den 1345): **rot** bei einer undokumentierten
  Abweichung **und** bei einem Eintrag, dessen Abweichung verschwunden ist
  („stale") — die Liste kann nicht verrotten. Vier weitere Tests fuer die
  Parser, Snapshot-Rundreise und die Klassifikation.

**Heute (G3-Tabelle):**

| | Anzahl |
|---|---:|
| literale Ganzzahl-`const` in `kernel/src/desk/` | **431** |
| gleich EmuTOS | **266** |
| abweichend (dokumentiert / undokumentiert) | **2** (2 / 0) |
| ohne gleichnamiges EmuTOS-`#define` (ungeprueft) | **163** |
| mehrdeutig (EmuTOS definiert mehrere Werte) | 0 |
| veraltete Abweichungseintraege | 0 |

| Name | unser Wert | Ort | EmuTOS | Art |
|---|---:|---|---|---|
| `DEOK` | 14 | `kernel/src/desk/inf.rs:2551` | 16 (`desk/desk_rsc.h:91`) | **intended** — About-Box ohne Autorenzeile, Anwendervorgabe 2026-10-09 (`5f819b96`) |
| `BLKDEVNUM` | 16 | `kernel/src/desk/main.rs:1471` | 26 (`include/sysconf.h:43`) | **defect** — Doc-Zitat `bdosdefs.h` falsch (dort nur benutzt, `:62`); **in U5a nicht behoben** (Auftrag) |

Deckt sich mit dem Prototyp des Reviews (431/266/2/163).

### 1.3 G4 — `tools/desk-cite-list.sh`

Listet jede `fn` des Desktops (rekursiv, gleiche Quellsuche wie die Probe)
ohne EmuTOS-Fundstelle `name.c:N`/`.h:N`/`.S:N` im Kommentarblock direkt
darueber (zusammenhaengende `///`-, `//`- und `#[…]`-Zeilen). Konvention fuer
gewollt neue Funktionen: **`// NEWTOS: <warum>`** im selben Block — sie
zaehlen als `newtos`, nicht als `uncited`. `--since <rev>` liefert genau die
Liste, die eine Quittung tragen muss (Funktionen, deren Spanne eine
geaenderte Zeile seit `<rev>` enthaelt; neue unverfolgte Dateien ganz).

**Heute:** `DESK-CITE: fns=530 cited=346 newtos=0 uncited=184` (deckt sich mit
dem Prototyp, 65 % / 35 %). U5a beruehrt keine Desktop-`fn`:
`--since HEAD` → 0. Die 184 sind Kandidaten, kein Befund (Review §4.3).

**Dokumentiert:** `docs/api/desktop/_rebuild-plan.md` **§12 „Receipt guards"**
— Pflichtangaben jeder DESK43-Quittung (Probe TOTAL + UNSAFE,
EMUTOS-CONSTS-Zeile, DESK-CITE gesamt + `--since`, Orakel) mit
Vorlage-Block; dort steht auch die `// NEWTOS:`-Konvention.

### 1.4 G2-Vorarbeit — das Aufrufsequenz-Orakel

`kernel/src/dbg_qedtrace.rs` bekommt einen **Sequenzmodus**, eingeschaltet
nur ueber das neue Kernel-Feature **`svc-seq-trace`** (Standard aus):

* jede Ausfuehrung in Abschlussreihenfolge — **keine** Umleitung der
  Geometrie-Aufrufe in den Prolog-Puffer (U1 §5 Punkt 2), kein Leeren je
  `evnt_multi`, kein Tastendruck-Riegel (QED-Logik bleibt fuer den QED-Modus);
* Aufruferspalte: `K` (Kernel-Raum = EL1-Desktop), `U<slot>` (EL0-Prozess);
* `>> evnt_multi entered (#n)` bei jedem Eintritt (ein headless nie
  zurueckkehrendes `evnt_multi` hat sonst keine Zeile);
* 512 KiB Puffer, bei jedem `evnt_multi`-Eintritt komplett nach
  `C:\SVCSEQ.LOG`;
* Opcodes 31/32/35 (`menu_icheck`/`menu_ienable`/`menu_register`) benannt —
  U1 §5 Punkt 1, „eine Zeile Arbeit"; ohne sie ist die Sequenz unlesbar.

Tor: `SVC_TRACE = QED_SVC_TRACE || SEQ_MODE` (beide Haken in
`exceptions.rs`), beide Modi per `const`-Assertion exklusiv. Host-Werkzeug
`tools/svc-seq.py`: Log → diffbare Sequenz (`--caller K`, `--no-caller`,
`--no-results`, `--no-vdi`, `--details`).

**Gezeigt mit einem QEMU-Lauf** (`method/seq-run.sh`): Scratch-Kopie +
Tasteninjektor nur in der Kopie (`method/inj-desk-seq.py`, scharf bei
`[desk] desktop up` — T10-Umgehung wie `arch03-4/desk_arm_patch.py` —,
Alt-C nach 1 s), Release rpi4 mit `--features svc-seq-trace`, MEM05-Plain-Karte
(Kopie, vorher ohne `SVCSEQ.LOG` geprueft). Ergebnis
`trace/SVCSEQ.LOG` (684 Zeilen) → `trace/sequence-all.txt` **551 Aufrufe**,
davon AES/GEMDOS/XBIOS des Desktops 246
(`trace/sequence-desktop-K-novdi-shape.txt`): Anlauf wie U1 (`appl_init`,
`wind_get`, 8×`objc_add`, `Ssystem`, `wind_calc`, `shel_get`,
`Fopen/Fread/Fclose`, …, 31×`menu_ienable`, 8×`menu_icheck`, …,
`>> evnt_multi`), dann der Alt-C-Zyklus mit `wind_create`, `Fsfirst`+83
`Fsnext`, `wind_set`, `wind_open`, `wind_get` **an ihrer Stelle im Strom**,
dann der Redraw-Zyklus. Screendump `seq-rpi4-t45.png`: Fenster `C:\` offen.

---

## 2. Geaenderte / neue Dateien

| Datei | Was |
|---|---|
| `tools/desk-drift-probe.sh` | Probe v2 (§1.1) |
| `tools/desk-cite-list.sh` (neu) | G4 (§1.3) |
| `tools/xtask/src/emutos_consts.rs` (neu) | G3: Parser, Vergleich, `--update`, 5 Tests |
| `tools/xtask/src/main.rs` | `mod emutos_consts`, Unterbefehl `emutos-consts`, Usage |
| `tools/xtask/emutos-defines.snapshot` (neu) | EmuTOS-Snapshot (generiert) |
| `tools/xtask/emutos-consts-deviations.txt` (neu) | DEOK, BLKDEVNUM |
| `kernel/src/dbg_qedtrace.rs` | Sequenzmodus, `SVC_TRACE`, `caller_tag`, 3 Opcode-Namen, Modul-Doc |
| `kernel/src/arch/aarch64/exceptions.rs` | beide Haken auf `SVC_TRACE` (Kommentar zeilengleich) |
| `kernel/src/lib.rs` | Kommentar am `dbg_qedtrace`-Modul (zeilengleich) |
| `kernel/Cargo.toml` | Feature `svc-seq-trace = []` |
| `tools/svc-seq.py` (neu) | Log → Sequenz |
| `docs/api/desktop/_rebuild-plan.md` | §12 Receipt guards |
| `measurements/measurement-2026-10-14-desk43-u5a/` (neu) | Evidenz |

---

## 3. Bau, Clippy, Tests (`checks/u5a-*`, frisch, beide Boards)

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1340/0/1 + neue | **1345/0/1** (+5 neue) |
| Release-Warnungen rpi4 / qemu-virt (tos-api/kernel) | 64/94 | **64/94** beide, 0 Fehler |
| Clippy rpi4 / qemu-virt | 76/125 | **76/125** beide |
| Driftprobe (v2) | 31 | **31** |
| SDK-Hosttests | 20/0/0 | **20/0/0** |
| mit `--features svc-seq-trace` | – | Bau 64/94, Clippy 76/125 beide Boards (kein neuer Fund; `dbg_qedtrace.rs` `CYCLES_AFTER_KEY` unbenutzt ist alt) |
| xtask-Clippy | – | kein Befund in `emutos_consts.rs` (8/1 vorbestehend in anderen Dateien) |

**Kein Produktunterschied, byteweise:** `kernel8-u5a.img` gegen MEM05
`kernel8-acc.img` (9ab10074; bis 401b1bc0 keine Kernel-/tos-api-Aenderung):
20 Byte = zwei Rev-Stempel + 2 Byte Baudatum; mit
`SOURCE_DATE_EPOCH` = 2026-10-13 neu gebaut: **16 Byte = nur die Rev-Stempel**.
QEMU raspi4b, Standardlauf, Plain-Karte: Desktop-Screendump t25
**byte-identisch** zu `measurement-2026-10-13-mem05-acceptance/screendumps/acc-on-rpi4-desktop-t25.png`
(sha256 `0e6f7704…324b`).

---

## 4. Guards dieser Einheit (Vorlage `_rebuild-plan.md` §12)

```
Guards (docs/api/desktop/_rebuild-plan.md §12):
  DRIFT-PROBE TOTAL: 31 (before: 31)   DRIFT-PROBE UNSAFE: 24 (baseline 24)
  EMUTOS-CONSTS: total=431 matched=266 mismatched=2 documented=2 undocumented=0 stale=0 no-counterpart=163
  DESK-CITE: fns=530 cited=346 newtos=0 uncited=184            (whole desktop)
  DESK-CITE --since 401b1bc0: none (U5a touches no desktop fn)
  Oracle: built here; one run, 551 calls, measurements/measurement-2026-10-14-desk43-u5a/trace/
```

Beruehrte `fn` ohne EmuTOS-Zitat (ausserhalb `desk/`, plan v2 §5 (c)):
`dbg_qedtrace::{pre_dispatch, post_dispatch, record, record_aes, flush, aes_name, caller_tag}`
— Debug-Werkzeug, kein TOS-Verhalten; `xtask::emutos_consts::*`,
`desk-drift-probe.sh`, `desk-cite-list.sh`, `svc-seq.py` — Host-Werkzeuge.

---

## 5. Offen

1. **`BLKDEVNUM = 16` statt 26** (`kernel/src/desk/main.rs:1471`) steht als
   `defect` in der Abweichungsliste — beheben in einer eigenen Einheit; der
   Hosttest erzwingt dann das Loeschen der Zeile.
2. **163 Konstanten ohne EmuTOS-Gegenstueck** bleiben ungeprueft; ebenso
   nicht-literale Konstanten (`A | B`, Casts) und EmuTOS-`enum`s. Die
   Konsolidierung (ein `pub`-Satz) ist U5b.
3. **Zitat-Liste ist eine Heuristik**: 184 Kandidaten; `// NEWTOS:` ist noch
   an keiner Funktion gesetzt (bestehende Funktionen nachzumarkieren war nicht
   Auftrag). `--since` ordnet Zeilen zwischen zwei `fn` der vorderen zu
   (Ueberzaehlung, nie Unterzaehlung).
4. **Orakel, Grenzen:** Aufzeichnung bei *Abschluss* eines Aufrufs; bei
   `Pexec`/`Pterm` kann die Aufruferspalte schon den naechsten Prozess nennen;
   Ergebnisse enthalten Kernel-Adressen (`XBIOS 0x10` Keytbl) → fuer den
   U9-Diff `--no-results`; 512 KiB, danach `TRUNCATED`; die Datei wird bei
   jedem `evnt_multi` ganz neu geschrieben (Kosten in langen Sitzungen; der
   QED-Modus hat auf echter Pi schon einmal gehangen, `tos-api/src/aes/trace.rs:38`)
   → Orakel fuer QEMU gedacht, nicht fuer Pi-Laeufe.
5. **Injektor-Bewaffnung (T10)** bleibt ein Scratch-Patch (`method/inj-desk-seq.py`);
   fuer U9 muss T10 entschieden sein (Review §2.5/6).
6. **`aes-trace`** (U1 §5 Punkt 3) unveraendert; nicht noetig, das Orakel
   spuelt ohne Eingabe.
7. Probe-Gruppe „Kernel bettet Desktop-Blob ein" (Review §3.2 Fussnote 2) ist
   **nicht** vorbereitet — sie entsteht erst mit U6b/U9.

## 6. Beobachtungen ausserhalb des Auftrags (nicht angefasst)

* `kernel/src/dbg_qedtrace.rs`: `const CYCLES_AFTER_KEY` ist unbenutzt (vorbestehende Warnung, Teil der 94).
* `hdiutil attach -readonly -nomount` meldet sich als veraltet (macOS); die Messskripte funktionieren noch.
