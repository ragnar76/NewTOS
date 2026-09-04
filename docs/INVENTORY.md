# INVENTORY — was umgesetzt ist, was fehlt

> Stand: 2026-09-04, Branch `desk-rebuild`, HEAD `e837a25`.
> Erhoben durch Abgleich dreier Quellen: der Spezifikation (`docs/api/`,
> 188 Dokumente, aus dem EmuTOS-Quelltext extrahiert), der **Dispatch-Tabellen
> im Code** und der Selbstauskunft (`docs/STATUS.md`, `docs/analysis/`).
>
> Diese Datei ist eine **Bestandsaufnahme, kein Änderungsprotokoll**. Sie
> beschreibt einen Zustand zu einem Datum und wird nicht fortgeschrieben,
> sondern neu erhoben.

**Inhalt.** §0 Beweisregeln · **§1 Behauptung ohne Deckung (19 Stück — das
eigentliche Ergebnis)** · §2 GEMDOS · §3 BIOS · §4 XBIOS · §5 VDI · §6 AES ·
§7 Desktop/Shell · §8 SDK/Toolchain · §9 Hardware-Schicht · §10 Zahlen ·
§11 Einschätzung bis Oktober · §12 Einschränkungen dieser Erhebung.

---

## 0. Was hier als Beleg zählt — und was nicht

Der Auftrag zu dieser Inventur enthielt eine ausdrückliche Vorgabe des Users:
**`docs/STATUS.md` und die Dokumente unter `docs/analysis/` haben keine
Beweiskraft.** Sie werden hier nirgends als Beleg für eine Einstufung zitiert,
sondern ausschließlich als *Behauptung*, die gegen den Code geprüft wird.

Der Grund ist strukturell: `STATUS.md` wird von dem geschrieben, der gerade
etwas landet, über die eigene Arbeit, im Moment des Landens. Danach liest es
niemand mehr gegen den Code. Es ist ein Änderungsprotokoll im Gewand eines
Zustandsberichts. Dieselbe Eigenschaft haben die Audit-Dokumente unter
`docs/analysis/`: sie beschreiben den Stand am Tag ihrer Erstellung.

Als Beleg gilt hier nur:

| Belegart | Was davon existiert |
|---|---|
| Eintrag in einer Dispatch-Tabelle | vollständig ausgewertet, siehe je Gruppe |
| Host-Test (`cargo test -p tos-api`) | 190 Tests, **am 2026-09-04 ausgeführt** — 175 grün, 15 rot |
| Unit-Test (`#[cfg(test)]`) | in den 190 enthalten (8 in `tos-api/src`); dazu 6 in `kernel/src`, die kein Testbefehl ausführt |
| QEMU-Selftest (`kernel/src/selftest_*.rs`) | 22 Module — **keines wird im Boot ausgeführt**, siehe §0.2 |
| Screendump | einer **am 2026-09-04 neu erzeugt**, siehe §7 |
| Hardware-Lauf | `docs/HW-TEST-LOG.md`: **2 Einträge, beide vom 2026-06-17** |

### 0.1 Die Testlage, gemessen statt behauptet

```
$ cargo test -p tos-api            # 2026-09-04
  tos_api (lib unit tests)  8 ok
  aes_button_dispatch       1 ok
  aes_desk_background       6 ok
  aes_dialog_undraw         2 ok
  aes_object_font           6 ok
  aes_window_content        1 ok
  aes_window_drag           4 ok
  gemdos_drive_login        2 ok
  gemdos_fs_paths           2 ok
  vdi_reference           143 ok / 15 FAILED   <-- rot
```

Die 15 roten Fälle sind Abweichungen gegen **auf echter Hardware gemessenes**
EmuTOS-Verhalten (`tosdump/`, EmuTOS auf einem Atari TT). Details in §5.

### 0.2 Die Selftests laufen nicht

`kernel/src/lib.rs:346-356` ersetzt den früheren Selftest-Block durch einen
Kommentar: *„Development self-tests are no longer invoked during the normal
boot."* Ein `grep` über alle Aufrufstellen bestätigt es: außerhalb von
`kernel/src/aes_demo.rs` (selbst nicht aufgerufen) ruft nichts ein
`selftest_*`-Modul auf.

**Folge für diese Inventur:** ein Selftest belegt, dass eine Funktion *einmal*
gemessen wurde, als jemand ihn von Hand einhängte. Er ist **kein stehender
Nachweis** und fängt keine Regression. Eine Funktion, für die es nur einen
Selftest gibt, steht unten deshalb unter **„verdrahtet, unbelegt"**, mit dem
Selftest als Vermerk „dormant" — nicht unter „umgesetzt".

### 0.3 Der Testbefehl des Projekts führt keine Tests aus

```
$ cargo xtask test            # 2026-09-04
running 0 tests ... ok
running 0 tests ... ok
running 0 tests ... ok
```

`tools/xtask/src/main.rs:559-568` beschränkt `cmd_test` auf
`--package tos-types --package xtask` — beide haben **null** Tests. Die 190
echten Tests in `tos-api` werden vom Testbefehl des Projekts nie angefasst.
Es gibt außerdem keine CI: `.github/workflows/` enthält nur eine `.gitkeep`.

Das erklärt mechanisch, wie die zwei Regressionen aus §1.1 unbemerkt bleiben
konnten.

---

## 1. Behauptung ohne Deckung

Das eigentliche Ergebnis dieser Inventur. Zwölf Stellen, an denen die
Dokumentation etwas sagt, das der Code nicht hält — oder an denen ein
Kontrollmechanismus, auf den sich das Projekt beruft, nicht existiert.
Jede ist gegen den Code oder durch eine am 2026-09-04 durchgeführte Messung
belegt.

### 1.1 Die VDI-Konformitätssuite ist rot, und zwei Fälle sind eine unbemerkte Regression — **BELEGT (gemessen)**

`docs/STATUS.md:243` führt Phase 12 als *„✅ KOMPLETT — alle ~70 funktionalen
Opcodes … Per QMP-Screendump end-to-end verifiziert (8/8 Pixel-Checks)"*.

Gemessen am 2026-09-04:

```
$ cargo test -p tos-api --test vdi_reference
test result: FAILED. 143 passed; 15 failed
```

Die Suite (`tos-api/tests/vdi_reference.rs`, Fälle in
`tos-api/tests/vdi_reference/cases_r2.rs` / `cases_r3.rs`) vergleicht nicht
gegen eine Meinung, sondern gegen **auf einem Atari TT gemessenes
EmuTOS-Verhalten** (`tosdump/`). Der Harness-Kommentar sagt es selbst
(`tos-api/tests/vdi_reference/harness.rs:5-7`): *„a difference here has exactly
one reading: NewTOS is wrong."*

**Zwei der 15 sind eine Regression, keine Altlast.** Ich habe die Suite in
einem separaten Worktree am Stand `5a475eb` (2026-08-30, der Commit, der die
Zahl zuletzt nannte) ausgeführt:

| Fall | bei `5a475eb` | bei HEAD `e837a25` |
|---|---|---|
| `cases_r2::case_0140 v_get_pixel top left corner` | **ok** | FAILED (`intout[1]` = −1, Referenz 0) |
| `cases_r2::case_0141 v_get_pixel bottom right corner` | **ok** | FAILED (`intout[1]` = −1, Referenz 0) |
| `cases_r3::case_0140 v_get_pixel inside the drawn box` | FAILED, **1** Feld falsch | FAILED, **2** Felder falsch |
| Gesamt | 145 ok / 13 rot | 143 ok / 15 rot |

Verursacher ist `a3563cb` *„fix(vdi): v_get_pixel takes the truecolor path, and
the count goes down"* (`tos-api/src/vdi/inquiry.rs:174-191`). Der Commit trägt
„fix" im Betreff, verschlechtert aber **jeden** Fall, den er anfasst: der Fall,
den er reparieren sollte, hat danach zwei falsche Felder statt einem, und zwei
vorher grüne Fälle sind rot. Als Verifikation nennt der Commit-Body
*„cargo xtask build … exit 0"* — also einen Build, keinen Test.

**Was ich nicht behaupte:** dass alle 15 Fälle Bugs sind. Die Referenz wurde auf
einem 4-Ebenen-TT gemessen, NewTOS ist per Entscheidung eine 16-Bit-Truecolor-
Workstation; ein Teil der Differenzen (Farbanzahl, Palettenpräzision,
Font-Namen) ist plausibel Profilunterschied. **Der Befund ist:** niemand hat
entschieden, welche Roten akzeptiert sind. Der Harness hat keine Skip-Liste,
keine „expected failure"-Markierung und keine Notiz im Code. Eine rote Suite
ohne Adjudikation ist kein Nachweis, sondern das Gegenteil.

### 1.2 Der Testbefehl des Projekts führt null Tests aus — **BELEGT (gemessen)**

`tools/xtask/src/main.rs:559-568`:

```rust
fn cmd_test(args: &[String]) -> Result<(), String> {
    …
    c.args(["test", "--package", "tos-types", "--package", "xtask"]);
```

Beide Pakete enthalten **null** `#[test]`. `cargo xtask test` meldet dreimal
„0 tests … ok" und Exit 0. Die 190 echten Tests werden nie ausgeführt.

`CLAUDE.md:266` nennt außerdem `cargo xtask qemu-test` als den Weg, die
Integrationstests zu fahren. Dieses Unterkommando existiert nicht — die
Match-Arme in `tools/xtask/src/main.rs:28-38` sind `build`, `gen-font`,
`gen-fsel-rsc`, `run`, `test`, `help`.

`.github/workflows/` enthält nur eine `.gitkeep` — es gibt keine CI.

### 1.3 „Selftest grün" belegt nichts Laufendes — **BELEGT (Code)**

22 Selftest-Module liegen in `kernel/src/`. `kernel/src/lib.rs:346-356`:
*„Development self-tests are no longer invoked during the normal boot."*
Ein `grep` über alle Aufrufstellen findet außerhalb von `kernel/src/aes_demo.rs`
(das selbst nirgends aufgerufen wird) keinen einzigen Aufruf.

`STATUS.md` beruft sich an Dutzenden Stellen auf „Selftest grün" als Aussage
über den **Zustand**. Es ist eine Aussage über einen **Zeitpunkt**. Nichts
verhindert, dass ein Selftest heute fehlschlagen würde; niemand würde es sehen.

### 1.4 `Super` (GEMDOS 0x20) — die Begründung für die Lücke ist falsch — **BELEGT (Quelle)**

`docs/STATUS.md:371` (2026-09-02) rechtfertigt das Fehlen so:
*„`Super` deckt sich mit EmuTOS (`bdosmain.c:221` + `ni()`)"*.

Das ist nachweislich falsch, und der Gegenbeweis steht in unserer **eigenen**
Spezifikation. `docs/api/gemdos/process/20-super.md:19-26`:

> `Super` ist **nicht** ein `funcs[]`/`osif()`-Eintrag. Es wird direkt im
> Trap-1-Assembler-Trampolin abgefangen, bevor der C-Dispatcher überhaupt
> aufgerufen wird.

Quelle: `references/emutos/bdos/rwa.S:159` (`_enter`) prüft die Funktionsnummer
vor allem anderen; `:278` ist der 0x20-Handler. Der `NI`-Eintrag bei
`bdosmain.c:221` wird für 0x20 **nie erreicht**.

NewTOS hat 0x20 in keiner Tabelle (`tos-api/src/gemdos/mod.rs:42-109`) und
nicht in der Frame-Sonderbehandlung (`kernel/src/arch/aarch64/exceptions.rs:283-321`)
→ `EINVFN` (−32). EmuTOS liefert stattdessen den alten Stackzeiger. Ein
Programm mit dem klassischen Idiom `long ssp = Super(0); … Super(ssp);`
bekommt −32 als „alten Stack" zurückgereicht.

### 1.5 XBIOS `Supexec` (0x26) meldet Erfolg und führt nichts aus — **BELEGT (Code)**

`tos-api/src/xbios/mod.rs:130-136`: jede nicht verdrahtete Funktionsnummer
fällt in `other => other as i64`. Für `Supexec` heißt das: Rückgabe **38**
(0x26), die übergebene Funktion wird nie aufgerufen.

`STATUS.md:242` nennt das *„ehrlich-unimpl via Funktionsnummer"*. Für die
BIOS/XBIOS-Konvention allgemein stimmt das (EmuTOS' `xbios_unimpl` tut
genau das). Für `Supexec` stimmt es nicht: dessen Vertrag ist *„gib zurück,
was `fn` zurückgab"*. 38 ist ein plausibler Rückgabewert und für den Aufrufer
von einem Erfolg nicht zu unterscheiden. `Supexec` ist zudem **keine**
Atari-Hardware-Funktion — es ist ein reiner CPU-Modus-Mechanismus, für den
NewTOS ein Äquivalent bauen könnte.

### 1.6 `Setexc(etv_timer)` / `Setexc(etv_critic)` werden angenommen und nie aufgerufen — **BELEGT (Code)**

`tos-api/src/bios/vectors.rs:92-107` speichert jeden Vektor und liefert den
alten zurück — der vollständige, TOS-treue `Setexc`-Vertrag. Aber:

* `ETV_TERM` (0x102) wird zugestellt — der **einzige** Konsument der
  Schattentabelle ist `kernel/src/arch/aarch64/exceptions.rs:804-809`.
* `ETV_TIMER` (0x100) und `ETV_CRITIC` (0x101) werden **nirgends** aufgerufen.
  `vectors::vector()` (`tos-api/src/bios/vectors.rs:173`) hat genau eine
  Aufrufstelle im ganzen Baum, und die liest `ETV_TERM`. Die anderen beiden
  Slots werden geschrieben und nie gelesen.
  `kernel/src/arch/aarch64/exceptions.rs:798` sagt es zu:
  *„etv_critic (M1) and etv_timer are later cooperative-kernel steps."*

Ein Programm, das einen Tick-Handler oder einen Disk-Fehler-Handler
installiert, bekommt den alten Vektor zurück wie unter TOS, hat also **keine
Möglichkeit zu bemerken**, dass sein Handler nie läuft.

### 1.7 `docs/HW-TEST-LOG.md` deckt keine der Hardware-Behauptungen — **BELEGT (Datei)**

`CLAUDE.md:267`: *„Hardware tests on RPi4 are manual, documented in
`docs/HW-TEST-LOG.md`."* Die Datei hat **zwei** Log-Einträge, beide vom
2026-06-17, der dritte ist `_pending_`.

`STATUS.md` behauptet danach mindestens: USB-HID auf echter HW (2026-07-02),
AES/Desktop auf echter HW (2026-07-12), TeraDesk-Lauf + GEMDOS-Messläufe
(2026-09-02), 4-Laufwerks-Erkennung (2026-06-18). Keiner davon steht im Log.

Ich sage **nicht**, dass diese Läufe nicht stattgefunden haben. Ich sage: es
gibt keinen Datensatz, gegen den man sie prüfen könnte, und der Ort, den das
Projekt selbst dafür benennt, ist leer.

### 1.8 CLAUDE.mds Testregel ist nirgends erfüllt — **BELEGT (Zählung)**

`CLAUDE.md:268-270`: *„Every TOS API function gets at least one test that
verifies the documented return value for documented input, and one that
verifies the documented error code for documented error input."*

Gezählt: ~230 verdrahtete API-Funktionen (GEMDOS 52 + BIOS 12 + XBIOS 33 +
VDI 67 + AES 79), 190 Tests, davon 158 allein VDI-Referenzfälle. **BIOS und
XBIOS haben null Host-Tests.** Die AES hat ~12 der 79 Opcodes in Host-Tests.
Die Regel ist nicht annähernd erfüllt und es gibt keinen Mechanismus, der sie
prüft.

### 1.9 STATUS Phase 13 nennt zwei Dinge offen, die im Code fertig sind — **BELEGT (Code)**

`docs/STATUS.md:244` führt als *„Verbleibend deferred (ehrlich, kein Stub)"*:

* *„`rsrc_saddr` No-op-Stub (Rule-2, separat)"* → `tos-api/src/aes/resource.rs:1299-1321`
  ist eine vollständige Implementierung: `get_addr`-Lookup, ABI-breiten
  Zeiger-Store, `1`/`0`-Rückgabe.
* *„#59 Submenu (`menu_attach`/`menu_istart`/Popup brauchen die
  `SMIB`-Registry-Infrastruktur)"* → die Registry existiert
  (`SUBMENU_TAB`, `smib_find_sub`), `menu_attach` ist bei
  `tos-api/src/aes/menu.rs:1379-1404` mit allen drei `ME_*`-Zweigen,
  `menu_istart` bei `:1417`, `menu_popup` bei `:964`.

Die Richtung ist hier untertreibend statt übertreibend — für die Verlässlichkeit
des Dokuments macht das keinen Unterschied.

### 1.10 STATUS Phase 14 beschreibt einen Desktop, der nicht mehr existiert — **BELEGT (Code)**

`docs/STATUS.md:245` beschreibt `desktop::run()` als Boot-Anwendung und
`kernel/src/desktop.rs` + `desktop_icons.rs` als „deprecated (bleiben bis
Feature-Parität in Git)". Beide Dateien sind seit `25deb42` gelöscht. Der
Boot-Pfad ist `shell_loop::sh_main(isauto, isgem)` (`kernel/src/lib.rs:391`),
der Desktop liegt in `kernel/src/desk/` (15 Dateien, 9279 Zeilen).

Ebenso spiegelt die Zeile nicht die spätere User-Entscheidung wider, dass
**TeraDesk** der Desktop sein soll (Memory `teradesk-is-the-desktop`), was die
gesamte „Offen"-Liste dieser Zeile neu bewertet.

### 1.11 STATUS Phase 02 zählt 12 von 15 Konsolenfunktionen — es sind 15 — **BELEGT (Code)**

`docs/STATUS.md:233`: *„D1–D5 done (12/15 Konsolen-Funktionen)"*.
`tos-api/src/gemdos/mod.rs:45-59` verdrahtet alle 15 (0x01–0x0B, 0x10–0x13),
einschließlich der drei blockierenden Eingabefunktionen, die 2026 auf
Phase 06 gewartet hatten.

### 1.12 GEMDOS 0x4D und 0x11A sind implementiert, ohne Spezifikation — **BELEGT (Code)**

`CLAUDE.md:119-132` (Regel 5, *Spec before code*) verlangt einen Eintrag in
`docs/api/`, bevor implementiert wird.

* **`Pwait` (0x4D)** — `kernel/src/arch/aarch64/exceptions.rs:309-312` →
  `handle_pwait` (`:660-678`). In EmuTOS ist 0x4D ein `NI`-Slot
  (`references/emutos/bdos/bdosmain.c:274`), gibt also `EINVFN`. NewTOS gibt
  0 zurück und führt einen Scheduler-Yield aus. Es gibt keine
  `docs/api/gemdos/**/4d-*.md`.
* **`Psigreturn` (0x11A)** — `:318-321`, eine NewTOS-Eigenerweiterung für den
  EL0-Upcall-Trampolin. Keine Spec-Datei.

Beides ist inhaltlich vertretbar (Erweiterung über TOS hinaus), aber es ist
undokumentiert und für 0x4D **abweichend von EmuTOS beobachtbar**.

### 1.13 Der `MAX_VERTICES`-Kommentar begründet den falschen Wert — **BELEGT (Quelle)**

`tos-api/src/vdi/primitives.rs:560-563` setzt das Eckpunkt-Limit auf 512 und
begründet es mit *„EmuTOS' `MAX_VERTICES` (`config.h`, make-512 profile = 512)"*.
`make 512` ist die **ROM-Größe**, nicht das Eckpunkt-Limit; `TARGET_512` kommt in
`config.h` gar nicht vor, und die Vorgabe für TOS3/TOS4 ist **1024**
(`references/emutos/include/config.h:1650-1656`). Die auf Hardware gemessene
Referenz bestätigt 1024. Ausführlich in §5.3 — es ist der einzige eindeutige
Funktionsdefekt, den diese Inventur neu gefunden hat.

---

## 2. GEMDOS (Trap 1)

**Dispatch-Tabelle:** `tos-api/src/gemdos/mod.rs:42-109` (statische Tabelle,
`MAX_FN = 0x57`, unbesetzter Slot → `EINVFN`).
**Rahmen-mutierende Sonderfälle:** `kernel/src/arch/aarch64/exceptions.rs:283-321`
(0x00, 0x31, 0x4B, 0x4C, 0x4D, 0x11A) — sie erreichen die Tabelle nie.
**MiNT-Erweiterung:** `Ssystem` (0x154) vor der Tabelle, `mod.rs:145-147`.

**Abgleich gegen EmuTOS.** EmuTOS' `funcs[]`
(`references/emutos/bdos/bdosmain.c:142-291`) plus der Assembler-Sonderfall
`Super` ergeben **54** Funktionsnummern. NewTOS verdrahtet **52** davon.

### 2.1 Umgesetzt (verdrahtet **und** belegt)

| ID | Funktion | Code | Prüfung |
|---|---|---|---|
| 0x1A | `Fsetdta` | `mod.rs:94` | Host-Test `gemdos_fs_paths.rs:55` (durch den echten Dispatcher, FAT16-Volume im Speicher) |
| 0x4E | `Fsfirst` | `mod.rs:96` | Host-Test, 10 Aufrufe inkl. `EFILNF`/`EPTHNF`/`ENMFIL`-Fehlerpfade (`gemdos_fs_paths.rs:62-97`) |
| 0x4F | `Fsnext` | `mod.rs:97` | Host-Test, inkl. Verzeichnisende `ENMFIL` |
| 0x3D | `Fopen` | `mod.rs:78` | Host-Test, inkl. „Ordner öffnen" → `EFILNF` |
| 0x3C | `Fcreate` | `mod.rs:86` | Host-Test `gemdos_fs_paths.rs` |
| 0x3E | `Fclose` | `mod.rs:79` | Host-Test |
| 0x41 | `Fdelete` | `mod.rs:88` | Host-Test |
| 0x0E | `Dsetdrv` | `mod.rs:100` | Host-Test + `gemdos_drive_login.rs` (Laufwerks-Login/Logout gegen Partitions-Rescan) |
| 0x47 | `Dgetpath` | `mod.rs:108` | Host-Test |
| 0x39 | `Dcreate` | `mod.rs:105` | Host-Test |

Zehn Funktionen. Alle zehn liefen am 2026-09-04 grün.

### 2.2 Verdrahtet, unbelegt

Für diese Funktionen gibt es einen Tabelleneintrag, aber **keinen laufenden
Nachweis** — kein Host-Test, kein aktiver Selftest, kein Screendump, der sie
zeigt.

| Gruppe | IDs | Code | Was an Prüfung existiert |
|---|---|---|---|
| Konsole (alle 15) | 0x01–0x0B, 0x10–0x13 | `mod.rs:45-59` | `kernel/src/selftest_console.rs` — **dormant** |
| Speicher | 0x14 `Maddalt`, 0x44 `Mxalloc`, 0x48 `Malloc`, 0x49 `Mfree`, 0x4A `Mshrink` | `mod.rs:64-68` | 6 Unit-Tests in `gemdos/memory.rs:318-386` prüfen die **Größen-Sonde** (`Malloc(-1)`-Rechnung), nicht den Trap-Pfad; die Pools initialisieren im Boot sichtbar (`[mm] BDOS pools:`) |
| Zeit | 0x2A–0x2D | `mod.rs:71-74` | `selftest_time.rs` — dormant |
| Version | 0x30 `Sversion` | `mod.rs:75` | `selftest_time.rs` — dormant |
| Datei, Rest | 0x3F `Fread`, 0x40 `Fwrite`, 0x42 `Fseek`, 0x43 `Fattrib`, 0x45 `Fdup`, 0x46 `Fforce`, 0x56 `Frename`, 0x57 `Fdatime` | `mod.rs:80-91` | keiner |
| DTA | 0x2F `Fgetdta` | `mod.rs:95` | keiner |
| Laufwerk | 0x19 `Dgetdrv`, 0x36 `Dfree` | `mod.rs:101-102` | keiner |
| Verzeichnis | 0x3A `Ddelete`, 0x3B `Dsetpath` | `mod.rs:106-107` | keiner |
| Prozess | 0x00 `Pterm0`, 0x31 `Ptermres`, 0x4B `Pexec`, 0x4C `Pterm` | `exceptions.rs:301-308` | `selftest_pexec.rs`, `selftest_pexec_d4d5.rs`, `selftest_d6.rs`, `selftest_d7.rs` — alle dormant. **Indirekt belegt:** der Boot startet über `Pexec` eine Anwendung (`[shell] sh_ldapp: starting NEWDESK`, Lauf 2026-09-04) |
| MiNT | 0x154 `Ssystem` | `mod.rs:145` | `selftest_cookie.rs` — dormant |

**Zahl: 43 Einträge verdrahtet-unbelegt** (42 aus der EmuTOS-Oberfläche + `Ssystem`).

Besonders auffällig: **`Fwrite` (0x40) und `Frename` (0x56) haben keinen
laufenden Test.** Beides sind Funktionen, die bei einem Fehler das Dateisystem
auf der Karte beschädigen. Der `Frename`-Zyklus-Bug, der in der Vergangenheit
Verzeichnis-Zyklen und verwaiste Cluster erzeugte, wäre von keinem heute
laufenden Test gefangen worden.

### 2.3 Ehrlich nicht unterstützt

| ID | Funktion | Begründung | Bewertung |
|---|---|---|---|
| 0x15 | `Srealloc` | Videl-spezifisch (`references/emutos/bdos/bdosmain.c:200-204`, `#if CONF_WITH_VIDEL`); Spec liegt vor (`docs/api/gemdos/15-srealloc.md`) | vertretbar: Falcon-Videl-Bildspeicher hat auf dem Pi kein Gegenstück. **Aber:** unser Zielprofil ist Falcon030/TOS4, wo EmuTOS es baut — also eine Entscheidung, keine Selbstverständlichkeit |
| — | FAT12-Schreiben | dokumentierte Entscheidung; Floppy ist außerhalb des Umfangs | vertretbar |
| 5–7 | `Pexec`-Modi `PE_BASEPAGE`, `PE_GOTHENFREE`, `PE_BASEPAGEFLAGS` | `tos-api/src/bdos/process/pexec.rs:175-177` → `UnsupportedMode` → `EINVFN` | **beobachtbare Lücke**, nicht HW-Absenz: EmuTOS implementiert alle drei. `PE_GOTHENFREE` (6) benutzen reale Shells |

### 2.4 Fehlt

| ID | Funktion | Wo es fehlt |
|---|---|---|
| 0x20 | `Super` | Spec liegt vor (`docs/api/gemdos/process/20-super.md`), keine Implementierung, kein Tabelleneintrag → `EINVFN`. Siehe §1.4 — die in `STATUS.md` genannte Begründung ist falsch |

### 2.5 Über EmuTOS hinaus (ohne Spec)

`Pwait` (0x4D) und `Psigreturn` (0x11A) — siehe §1.12.

### 2.6 GEMDOS in Zahlen

| | Anzahl |
|---|---|
| EmuTOS-Oberfläche (Funktionsnummern) | 54 |
| umgesetzt (verdrahtet + belegt) | **10** |
| verdrahtet, unbelegt | **42** (+ `Ssystem` = 43 Einträge) |
| ehrlich nicht unterstützt | **1** Funktion (0x15 `Srealloc`) + 3 `Pexec`-Modi |
| fehlt | **1** (0x20 `Super`) |
| über EmuTOS hinaus, ohne Spec | **2** (0x4D, 0x11A) |
| Behauptung ohne Deckung | **3** (§1.4, §1.11, §1.12) |

Kontrollsumme: 10 + 42 + 1 + 1 = 54.

---

## 3. BIOS (Trap 13)

**Dispatch:** `tos-api/src/bios/mod.rs:72-94` — ein `match` über die
Funktionsnummer. Alle **12** EmuTOS-Funktionsnummern 0x00–0x0B sind besetzt;
außerhalb des Bereichs gibt der Catch-all die Funktionsnummer zurück
(`mod.rs:92`), wie EmuTOS' Trampolin (`references/emutos/bios/vectors.S:1323`).

**Es gibt für den BIOS keinen einzigen Host-Test.** `kernel/src/selftest_bios.rs`
und `selftest_emmc.rs` existieren, laufen aber nicht (§0.2 / §1.3).

### 3.1 Umgesetzt (verdrahtet **und** belegt)

Belegt durch einen von mir am **2026-09-04** durchgeführten QEMU-Lauf
(`qemu-system-aarch64 -M raspi4b -drive file=<FAT16-Bild>,if=sd`) mit einem
Bild, das ich selbst gebaut habe (MBR + FAT16, eine 24-Byte-Datei `HELLO.TXT`,
ein Ordner `APPS`) — also mit **bekanntem Soll**:

| Funktion | Code | Was der Lauf beweist |
|---|---|---|
| MBR-/Partitionsscan | `tos-api/src/bios/drive.rs:57-110` | Serial: `[emmc] SD card on controller @ …`, danach `[desk] desktop up (1 drives)`; ohne Bild `(0 drives)` |
| BPB-Parser | `tos-api/src/bios/bpb.rs:200-240` | der Desktop liest die Wurzel korrekt — falsche Geometrie ergäbe Müll |
| Sektor-Cache/Blocklesen | `tos-api/src/bdos/fs/cache.rs:88` | siehe oben |

**Einschränkung, die ich ausdrücklich mache:** dieser Lauf beweist die
Block-/FAT-Schicht, **nicht die BIOS-Traps**. `drive::mount()`, `bpb` und der
Cache greifen direkt auf `host().block()` zu; `bios::dispatch` liegt daneben und
wird auf diesem Pfad nicht durchlaufen.

### 3.2 Verdrahtet, unbelegt

| ID | Funktion | Code | Anmerkung |
|---|---|---|---|
| 0x00 | `Getmpb` | `mod.rs:74` → `getmpb.rs` | |
| 0x01 | `Bconstat` | `mod.rs:75` | |
| 0x02 | `Bconin` | `mod.rs:76` | |
| 0x03 | `Bconout` | `mod.rs:77` | |
| 0x04 | `Rwabs` | `mod.rs:79` → `mod.rs:138-205` | logischer **und** physischer Modus (`RW_NOTRANSLATE`) implementiert |
| 0x05 | `Setexc` | `mod.rs:80` → `vectors.rs:92` | Vertrag vollständig; zwei der drei `etv_*`-Hooks werden nie aufgerufen, siehe §1.6 |
| 0x07 | `Getbpb` | `mod.rs:82` | |
| 0x08 | `Bcostat` | `mod.rs:78` | |
| 0x09 | `Mediach` | `mod.rs:83` | |
| 0x0A | `Drvmap` | `mod.rs:84` | |
| 0x0B | `Kbshift` | `mod.rs:85` → `mod.rs:106-117` | Get/Set über den IKBD-Dekoder |

**Zahl: 11 verdrahtet-unbelegt.**

### 3.3 Konstante Rückgaben, die *keine* Stubs sind

`TICKCAL => TICKCAL_MS as i64` (`mod.rs:81`, Konstante 20) sieht wie ein Stub
aus, ist aber korrekt: EmuTOS' `tickcal()` ist selbst
`return(20L);` (`references/emutos/bios/bios.c:1096-1099`). Keine Beanstandung.

### 3.4 Beobachtbare Lücke unterhalb der Dispatch-Tabelle

`Setexc(ETV_TIMER)` / `Setexc(ETV_CRITIC)` — siehe §1.6. Die Funktion ist
verdrahtet und vertragstreu; **der Effekt fehlt**. Ein Programm kann das nicht
merken. Das ist die gefährlichste Form einer Lücke: keine Fehlermeldung,
kein Rückgabewert, der sie anzeigt.

### 3.5 BIOS in Zahlen

| | Anzahl |
|---|---|
| EmuTOS-Oberfläche | 12 |
| umgesetzt (verdrahtet + belegt) | **0** über den Trap-Pfad |
| verdrahtet, unbelegt | **11** (+ `Rwabs`-Blockpfad indirekt belegt) |
| ehrlich nicht unterstützt | 0 |
| fehlt | 0 |
| Behauptung ohne Deckung | **2** (§1.6 `etv_*`, §1.8 Testregel) |

---

## 4. XBIOS (Trap 14)

**Dispatch:** `tos-api/src/xbios/mod.rs:64-137`, ein `match`; jede nicht
genannte Nummer fällt in `other => other as i64` (`:136`) und gibt sich selbst
zurück — die EmuTOS-`xbios_unimpl`-Konvention.

**EmuTOS-Oberfläche im Zielprofil.** Bei Falcon030/TOS4 (`CONF_WITH_DMASOUND`)
ist `LAST_ENTRY = 0x8d` (`references/emutos/bios/xbios.c:1273-1283`); die
Tabelle ab `:1285` hat **90+ echte** Einträge. NewTOS routet **33** Nummern.

### 4.1 Umgesetzt (verdrahtet **und** belegt)

| ID | Funktion | Code | Prüfung |
|---|---|---|---|
| 0x02/0x03 | `Physbase`/`Logbase` | `screen.rs:53-66` | Boot-Lauf 2026-09-04: der Framebuffer steht, der Desktop rendert (Screendump §7) |
| 0x04 | `Getrez` | `screen.rs:77-79` | dito — liefert konstant 4 (`TT_MEDIUM`), siehe §4.4 |
| 0x06/0x07 | `Setpalette`/`Setcolor` | `screen.rs:117-131,…` | Boot-Lauf: die 16 Pens im Screendump sind korrekt |

Drei Gruppen. Alles Weitere ist unbelegt.

### 4.2 Verdrahtet, unbelegt — echte Arbeit

`Setscreen` (0x05), `Cursconf` (0x15), `Rsconf` (0x0F), `Keytbl` (0x10),
`Random` (0x11), `Protobt` (0x12), `Settime`/`Gettime` (0x16/0x17),
`Setprt` (0x21), `Kbdvbase` (0x22), `Kbrate` (0x23), `Vsync` (0x25).
**12 Funktionen.** `kernel/src/selftest_xbios.rs` deckt sie ab — dormant.

### 4.3 Ehrlich nicht unterstützt — HW-Absenz mit Vertragswert

Das ist **kein Mangel**, sondern die richtige Antwort für Hardware, die es auf
dem Pi nicht gibt (CLAUDE.md Non-Goal: kein Atari-Hardware-Nachbau):

| IDs | Gruppe | Antwort | Code |
|---|---|---|---|
| 0x08, 0x09, 0x0A, 0x13, 0x29, 0x2A, 0x2B | Floppy (WD1772) + ACSI/DMA | `EUNDEV` (−15) | `mod.rs:79` |
| 0x0C | `Midiws` (MIDI-ACIA) | No-op | `mod.rs:87` |
| 0x14 | `Scrdmp` (Drucker) | No-op | `mod.rs:86` |
| 0x19 | `Ikbdws` (IKBD-ACIA) | No-op | `mod.rs:109` |
| 0x1C–0x1E, 0x20 | PSG (YM2149) | 0 / No-op | `mod.rs:112-115` |
| 0x2C | `Bconmap` | 0x2c „nicht verfügbar" | `mod.rs:88` |
| 0x40 | `Blitmode` | 0x0000 „kein Blitter" | `mod.rs:126` |
| 0x0D, 0x1A, 0x1B, 0x1F | MFP-68901 (`Mfpint`, `Jdisint`, `Jenabint`, `Xbtimer`) | Funktionsnummer | Catch-all |
| 0x50–0x57 | TT-Shifter (`Esetshift` …) | Funktionsnummer | Catch-all |
| 0x60–0x8D | DSP56001 + DMA-Sound | Funktionsnummer | Catch-all |

**16 Nummern mit ausdrücklichem Vertragswert** (7 Floppy/DMA, Midiws, Scrdmp,
`Ikbdws`, 4 PSG, Bconmap, Blitmode), **~47 über den Catch-all.**

### 4.4 Lücken, die *nicht* HW-Absenz sind

| ID | Funktion | Was passiert | Bewertung |
|---|---|---|---|
| **0x26** | `Supexec` | gibt **38** zurück, führt `fn` nie aus | **Behauptung ohne Deckung, §1.5.** Kein Atari-Hardwarebezug — ein reiner CPU-Modus-Mechanismus. Für den Aufrufer nicht von Erfolg zu unterscheiden |
| 0x00 | `Initmous` | Funktionsnummer | Spec liegt vor (`docs/api/xbios/00-initmous.md`). NewTOS *hat* eine Maus (USB-HID + IKBD-Dekoder) — die Registry fehlt, nicht die Hardware |
| 0x0E | `Iorec` | Funktionsnummer | Spec liegt vor (`0e-iorec.md`), Struktur-Spec ebenfalls (`docs/api/structures/iorec.md`). Der IKBD-Ringpuffer existiert |
| 0x18 | `Bioskeys` | Funktionsnummer | Spec liegt vor (`18-bioskeys.md`); die Tastaturtabellen existieren (`keytbl` ist implementiert) |
| 0x58–0x5F | Videl (`VsetMode`, `VgetMonitor`, `VsetSync`, `VgetSize`, `VsetRGB`, `VgetRGB`, `Vfixmode`) | Funktionsnummer | Grenzfall. Das erklärte Zielprofil ist **Falcon030/TOS4**, wo EmuTOS diese baut; NewTOS *hat* einen konfigurierbaren Framebuffer. Eine Falcon-bewusste Anwendung, die `VgetSize`/`VsetRGB` benutzt, bekommt Unsinn |
| 0x2E | `NVMaccess` | Funktionsnummer | Falcon-NVRAM; auf dem Pi kein Gegenstück (keine RTC-Batterie) — vertretbar |
| 0x05 | `Setscreen`-Bases | speichert `log`/`phys`, **niemand liest sie** | siehe unten |

**`Setscreen` / `Logbase` / `Physbase` — beobachtbare Falschauskunft.**
`tos-api/src/xbios/screen.rs:93-104` legt die übergebenen Basen in zwei
`AtomicU64` ab; `physbase()`/`logbase()` (`:53-66`) geben sie zurück. Ein
`grep` über den ganzen Baum findet **keinen weiteren Leser** von `PHYSBASE` /
`LOGBASE` — der VDI-Rasterisierer zeichnet unverändert in
`host().screen().base` (`kernel/src/vdi_backend.rs`).

*Fehlerszenario:* das klassische Doppelpuffer-Idiom
`Setscreen(buf, buf, -1); …zeichnen…; Setscreen(fb, fb, -1)`. NewTOS meldet
nach dem ersten Aufruf `Physbase() == buf` — der Bildschirm zeigt aber
weiterhin den echten Framebuffer, und die VDI-Ausgabe landet ebenfalls dort,
also **sichtbar statt verborgen**. Das Programm hat keine Möglichkeit, das zu
bemerken. **PLAUSIBEL** (aus dem Code hergeleitet, nicht gefahren).

**`Getrez` widerspricht dem VDI.** `Getrez` liefert konstant `4` = TT-Medium =
**16 Farben, 4 Ebenen** (`screen.rs:77-79`). Der VDI meldet über dieselbe API
`INQ_TAB[4] = v_planes = 16` und `DEV_TAB[13] = 256`
(`kernel/src/vdi_backend.rs:33,40`). Drei Antworten auf dieselbe Frage.
*Fehlerszenario:* eine Anwendung, die eine MFDB aus `Getrez()` dimensioniert
(`fd_nplanes = 4`) und sie an `vro_cpyfm` gibt, blittet gegen einen
16-Ebenen-Bildschirm. **PLAUSIBEL.**

### 4.5 XBIOS in Zahlen

| | Anzahl |
|---|---|
| EmuTOS-Oberfläche (Falcon/TOS4-Profil) | ~103 |
| umgesetzt (verdrahtet + belegt) | **5** |
| verdrahtet, unbelegt (echte Arbeit) | **12** |
| ehrlich nicht unterstützt (HW-Absenz) | **~63** |
| Lücke, die keine HW-Absenz ist | **11** (0x26, 0x00, 0x0E, 0x18, 0x58–0x5F) |
| Behauptung ohne Deckung | **3** (§1.5 `Supexec`, `Setscreen`-Basen, `Getrez`-Widerspruch) |

---

## 5. VDI (Trap 2, Selektor 115)

**Dispatch:** `tos-api/src/vdi/mod.rs:229-353`. Anders als GEMDOS/BIOS hat der
VDI keinen Fehlerkanal — ein unbekannter Opcode ist ein stiller No-op
(`mod.rs:349-352`), genau wie EmuTOS (`vdi_main.c:121-192`).

**Opcode-Abdeckung: vollständig.** Ich habe die Opcode-Tabelle aus
`docs/api/vdi/_dispatcher.md:250-333` (die selbst aus `vdi_main.c`s
`jmptb1`/`jmptb2` extrahiert ist) gegen das `match` gestellt: **jeder
funktionale Opcode** — 1–3, 5–9, 11–26, 28, 30–33, 35–39, 100–131, 134 — ist
verdrahtet. Nicht geroutet sind genau die Opcodes, die auch in EmuTOS
`vdi_v_nop` sind: 4, 10, 27, 29, 34, 132, 133. **Auf Dispatch-Ebene fehlt
nichts.**

### 5.1 Umgesetzt (verdrahtet **und** belegt)

**143 von 158 Referenzfällen grün**, gemessen am 2026-09-04. Die Referenz ist
kein Dokument, sondern ein Dump von echter Hardware
(`tosdump/reference/EMUTOS14-VDI-r2.LOG` / `-r3.LOG`). Die grünen Fälle decken
u. a. ab: `v_opnwk`/`v_opnvwk`/`v_clswk`, die komplette Attribut-Gruppe mit
ihren Klemmungen, `vs_clip`, `vqt_extent`/`vqt_width`/`vqt_fontinfo`,
`vql/vqm/vqf/vqt_attributes`, die sieben GDPs (`v_bar`, `v_arc`, `v_pieslice`,
`v_circle`, `v_ellipse`, `v_ellarc`, gerundetes Rechteck), `vq_color`,
`vsin_mode`, `v_escape`-Unterfunktionen.

Dazu: der Boot-Lauf 2026-09-04 (§7) zeigt Text, Icons, Dither-Muster,
Rahmen und den Mauszeiger korrekt gerendert — das ist der Beweis **unterhalb**
der Parameterblock-Ebene, den die Referenzfälle ausdrücklich nicht führen
(`harness.rs:9-14`: sie prüfen die fünf Arrays, keine Pixel).

### 5.2 Die 15 roten Fälle, einzeln adjudiziert

`STATUS.md:243` nennt Phase 12 „✅ KOMPLETT". Sie ist es nicht. Ich habe jeden
roten Fall gegen die EmuTOS-Quelle geprüft:

| Fälle | Differenz | Verdikt |
|---|---|---|
| r2/46 `vq_color` Pen 255 | 255 vs −1 | **Profilunterschied, NewTOS korrekt.** `DEV_TAB[13] = (v_planes<8) ? (1<<v_planes) : 256` (`vdi_control.c:206`) — bei 16 Ebenen ist Pen 255 gültig; die Referenz mass 4 Ebenen |
| r2/89 `vsm_color` 99 | 99 vs 1 | **Profilunterschied, NewTOS korrekt.** Gleiche Ursache über `validate_color_index` |
| r3/145 `intout[13]` | 256 vs 16 | **Profilunterschied, NewTOS korrekt** (`vdi_control.c:206`) |
| r3/117,119 `intout[4]` | 16 vs 4 | **Profilunterschied, NewTOS korrekt** (`INQ_TAB[4] = v_planes`, `vdi_control.c:210`) |
| r3/117,119 `intout[5]` | 0 vs 1 | **Profilunterschied, NewTOS korrekt** — EmuTOS setzt selbst 0, wenn `v_planes == 16` (`vdi_control.c:211-212`). Hier ist NewTOS *treuer* als die gemessene Referenz |
| **r3/117,119 `intout[14]`** | **512 vs 1024** | **ECHTER DEFEKT — siehe §5.3** |
| r3/145 `intout[39]` | 0 vs 4096 | **offen seit Monaten.** `DEV_TAB[39] = get_palette()` (`vdi_control.c:208`); bereits als „Mismatch #4" in `docs/api/vdi/workstation/100-v_opnvwk.md` notiert und nie behoben |
| r2/140,141 `v_get_pixel` Ecken | −1 vs 0 | **Regression** seit `a3563cb` (§1.1). Vorher grün |
| r3/140,142 `v_get_pixel` auf Inhalt | 0/0 vs 15/1 | **verschlechtert** durch `a3563cb`: vorher wich 1 Feld ab, jetzt 2 |
| r2/51 `vqin_mode` String | 0 vs 1 | **ungeklärt.** EmuTOS setzt alle vier Modi auf 0 (`vdi_mouse.c:575-578`), die Nachbarfälle Valuator/Choice sind **grün**, nur der String-Fall weicht ab. Entweder ist die Referenzmessung nicht der Build, für den wir sie halten, oder es gibt einen Schreibpfad, den ich nicht gefunden habe |
| r2/54–57 `vqt_name` `intout[33]` | Gift vs 0 | **ungeklärt.** Die Referenz schreibt ein 34. Wort, NewTOS 33 |
| r2/169 `v_dspcur` `intin[0]` | Gift vs 0 | **gering.** Echtes EmuTOS beschreibt das **Eingabe**-Array; wir lassen es unberührt. Nur für ein Programm sichtbar, das `INTIN` wiederverwendet |

**Bilanz: 7 Fälle sind Profilunterschied und kein Fehler, 1 ist ein echter
Defekt, 2 sind eine Regression, 2 sind eine bekannte, unbehobene Abweichung,
3 sind ungeklärt.**

Der eigentliche Befund ist nicht die Zahl, sondern: **niemand hat diese
Adjudikation vor mir gemacht.** Der Harness hat keine Skip-Liste und keine
„akzeptiert"-Markierung, also ist die Suite als Regressionsnetz wertlos —
jeder neue rote Fall geht im Rauschen der sieben Profilunterschiede unter.

### 5.3 `MAX_VERTICES` ist 512 statt 1024 — **ECHTER DEFEKT, BELEGT**

`tos-api/src/vdi/primitives.rs:560-563`:

```rust
/// fixed-size fill buffers. This is EmuTOS' `MAX_VERTICES` (`config.h`,
/// make-512 profile = 512); it is also advertised as `INQ_TAB[14]` …
pub(super) const MAX_VERTS: usize = 512;
```

Die Begründung im Kommentar ist falsch. Der Kommentar verwechselt **`make 512`**
(das 512-KB-**ROM-Abbild**) mit **`MAX_VERTICES 512`** (dem
TOS-2-Eckpunkt-Limit). Belegt aus der Quelle:

* `references/emutos/include/config.h:1650-1656`:
  *„TOS2 allows 512 vertices, TOS3/TOS4 allow 1024"* → `#define MAX_VERTICES 1024`.
* Die drei Stellen mit `512` (`config.h:388`, `:446`, `:549/561`) liegen
  **innerhalb** der Blöcke `#ifdef TARGET_192` (ab `:210`) und
  `#ifdef TARGET_256` (ab `:402`).
* **`TARGET_512` kommt in `config.h` überhaupt nicht vor** — ein `make 512`
  betritt keinen dieser Blöcke und bekommt den Vorgabewert **1024**
  (`references/emutos/Makefile:558-563` definiert nur `-DTARGET_512`).
* Die auf Hardware gemessene Referenz meldet `INQ_TAB[14] = 1024`.

Drei unabhängige Quellen sagen 1024, NewTOS sagt 512.

*Fehlerszenario:* `tos-api/src/vdi/primitives.rs:579` klemmt still
(`let n = n.min(MAX_VERTS);`). Ein Vektorzeichenprogramm oder ein Diagramm-Modul,
das ein Polygon mit 600 Eckpunkten füllt, bekommt eine Figur, die nach 512
Punkten zuklappt — ohne Fehlermeldung. Und `vq_extnd(1)` sagt der Anwendung
vorher 512, sodass eine gut erzogene Anwendung sich selbst unnötig beschränkt.

**Verdikt: Bug, keine akzeptable Abweichung.** Das ist beobachtbar durch zwei
dokumentierte Schnittstellen und hat keinen Hardwarebezug. Behebung ist eine
Konstante plus die Puffergrößen, die an ihr hängen.

### 5.4 VDI in Zahlen

| | Anzahl |
|---|---|
| funktionale Opcodes (EmuTOS, Falcon/TOS4-Profil) | 67 |
| verdrahtet | **67 (100 %)** |
| Referenzfälle grün | **143 / 158** |
| rot: Profilunterschied (kein Fehler) | 7 |
| rot: echter Defekt | 1 (`MAX_VERTICES`) |
| rot: Regression seit 2026-08-30 | 2 (+ 2 verschlechtert) |
| rot: bekannt, unbehoben | 1 (`DEV_TAB[39]`) |
| rot: ungeklärt | 5 |
| Behauptung ohne Deckung | **3** (§1.1 „KOMPLETT", §1.1 Regression, §5.3 falsche Kommentar-Begründung) |

---

## 6. AES (Trap 2, Selektor 200)

**Dispatch:** `tos-api/src/aes/mod.rs:345-436` (`crysbind`, das Gegenstück zu
EmuTOS' `aes/gemsuper.c:77-421`). Ein nicht getroffener Opcode setzt
`int_out[0] = -1` (`mod.rs:435`), wie EmuTOS' `default:`-Zweig.

**Opcode-Abdeckung: vollständig.** Ich habe alle `case`-Labels aus
`references/emutos/aes/gemsuper.c` extrahiert (79 Stück) und gegen das `match`
gestellt: **jeder einzelne ist verdrahtet**, einschließlich der
konfigurationsabhängigen (`menu_popup`/`attach`/`istart`/`settings`,
`objc_sysvar`, `appl_search`-Familie). Auf Trap-Ebene fehlt nichts.

### 6.1 Umgesetzt (verdrahtet **und** belegt)

**Aus Host-Tests** (32 grün am 2026-09-04, `tos-api/tests/aes_*.rs`) — diese
rendern durch den echten Dispatcher in einen Pixelpuffer und prüfen Pixel:

`appl_init` (10), `evnt_multi` (25), `objc_draw` (42), `form_dial` (51),
`graf_mkstate` (79), `fsel_input` (90), `wind_create/open/close/get/set/new`
(100–105, 109).

**Aus dem Boot-Lauf 2026-09-04** (Screendump §7). Der Desktop ist ein echter
AES-Client: `kernel/src/desk/mod.rs:104-115` setzt einen `svc #2` mit
Selektor 200 ab, kein interner Kurzschluss. Der gezeigte Bildschirm ist also
der Beweis für den ganzen Trap-Pfad:

`appl_init` (10), `menu_bar` (30), `objc_draw` (42), `graf_mouse` (78),
`wind_create/open/get/set` (100–105), `wind_calc` (108), `evnt_multi` (25),
dazu `rsrc_*` (die Icons im Fenster stammen aus der Ressource).

**Summe: 15 der 79 Opcodes haben einen laufenden Nachweis.**

### 6.2 Verdrahtet, unbelegt

**64 Opcodes.** Darunter alles, was Interaktion braucht und im Emulator ohne
Eingabe nicht auslösbar ist: `evnt_button/keybd/mouse/mesag/timer` (20–24),
`form_do/alert/error/keybd/button` (50, 52, 53, 55, 56), die sieben
`graf_*box`-Funktionen (70–76), `objc_edit` (46), `menu_popup/attach/istart`
(36–38), `scrp_*` (80–82), `shel_*` (120–127), `rsrc_*` (110–114).

**Wichtig für die Planung:** ich habe stichprobenartig **sechs** Punkte
nachgesehen, die im AES-Audit-Master als offen geführt werden, und alle sechs
sind im Code vorhanden:

| Punkt | Fundstelle |
|---|---|
| `rsrc_saddr` (angeblich Stub) | `tos-api/src/aes/resource.rs:1299-1321` — echte Implementierung |
| `scrp_clear` (angeblich Stub) | `tos-api/src/aes/shell.rs:239-300` — echte Verzeichnis-Löschschleife |
| `graf_slidebox` („Elevator tot", BLOCKER) | `tos-api/src/aes/graf.rs:423-480` — mit Grip-Offset und Klemmung |
| Submenüs (`menu_attach`/`menu_istart`) | `tos-api/src/aes/menu.rs:1379-1450`, `SUBMENU_TAB`-Registry vorhanden |
| Fileselector-Sortierung | `tos-api/src/aes/fsel.rs:442-480` — EmuTOS' Shell-Sort mit `fs_comp`-Schlüssel |
| Slider-Drag / `WM_HSLID`/`WM_VSLID`/`WA_*PAGE` | `tos-api/src/aes/window.rs:2516-2560, 2912` |

**Das heißt nicht, dass sie korrekt sind** — ich habe Vorhandensein geprüft,
nicht Treue. Es heißt: der Backlog im Audit-Master ist veraltet, und wer nach
ihm plant, plant Arbeit ein, die schon getan ist.

### 6.3 Ehrlich nicht unterstützt

| Opcode | Funktion | Code | Bewertung |
|---|---|---|---|
| 14 | `appl_tplay` | `appl.rs:412-414` — `set_ret(1)`, sonst nichts | **Regel-2-Grenzfall.** EmuTOS' `ap_tplay` ist `void` und lässt `ret = TRUE` stehen, der Rückgabewert stimmt also. Was fehlt, ist die Wiedergabe der aufgezeichneten Ereignisse — NewTOS hat keinen Pfad, Ereignisse in die AES-Eingabe zurückzuspeisen |
| 15 | `appl_trecord` | `appl.rs:402-404` — `set_ret(0)` | dito; `0` = „nichts aufgezeichnet" ist ein zulässiges Ergebnis |

Beides ist im Code als solches dokumentiert. Real benutzt es kaum eine
Anwendung; für Oktober irrelevant.

### 6.4 Fehlt

Nichts auf Opcode-Ebene. Was fehlt, sitzt **unter** den Opcodes: der
kooperative Multi-Prozess-Kern (mehrere AES-Prozesse parken/wecken) und die
Desk-Accessories. Beide sind Teil eines treuen EmuTOS (EmuTOS *ist*
kooperativ multitaskend), nicht Weiterentwicklung. Belegbar im Code ist der
Stand so:

* Die Substrat-Bausteine existieren: `sleep`/`wake`
  (`tos-api/src/bdos/process/sched.rs`), der EL0-Upcall
  (`kernel/src/arch/aarch64/exceptions.rs`, `Psigreturn` 0x11A), der
  S2-Kontextwechsel (`exceptions.rs:684-710` `apply_switch`), und die
  `evnt_*`-Waits geben einen `Switch` nach oben durch
  (`tos-api/src/aes/mod.rs:355-360`).
* Ein `.ACC`-Lader existiert **nicht** — `grep -rn "\.ACC"` findet nichts im
  Kernel.

### 6.5 AES in Zahlen

| | Anzahl |
|---|---|
| EmuTOS-Opcodes | 79 |
| verdrahtet | **79 (100 %)** |
| umgesetzt (verdrahtet + belegt) | **15** |
| verdrahtet, unbelegt | **64** |
| ehrlich nicht unterstützt | **2** (`appl_tplay`, `appl_trecord`) |
| fehlt (Opcode-Ebene) | **0** |
| fehlt (darunter) | Accessories-Lader; kooperativer Kern nur als Substrat |
| Behauptung ohne Deckung | **2** (§1.9, plus der veraltete WP-Backlog in §6.2) |

---

## 7. Desktop und Shell

Kein TOS-API-Vertrag, sondern eine **Anwendung** auf ihm. `kernel/src/desk/`,
15 Dateien, 9279 Zeilen; die Shell-Schleife in `kernel/src/shell_loop.rs`.

### 7.1 Belegt durch einen Lauf am 2026-09-04

Ich habe ein FAT16-Bild mit **bekanntem Inhalt** gebaut (eine 24-Byte-Datei
`HELLO.TXT`, ein Ordner `APPS`), `qemu-system-aarch64 -M raspi4b` damit
gebootet und den Bildschirm per QMP abgezogen. Ergebnis:

* Serial: `[emmc] SD card on controller @ …`,
  `[shell] sh_ldapp: starting NEWDESK, sh_nextapp=3, sh_isgem=1`,
  `[desk] desktop up (1 drives)` — ohne Bild: `(0 drives)`.
* Bildschirm: Menüleiste (Desk/File/View/Options), grüner `g_patcol`-Dither,
  Laufwerks-Icon links oben, Papierkorb rechts oben, ein Fenster `C:\` mit
  Titelleiste, Schließ-/Voll-Box, Infozeile, beiden Schiebern und Pfeilfeldern.
* Die Infozeile meldet **„2 items 24 bytes used"** — exakt die beiden Einträge
  und die exakte Byte-Zahl, die ich ins Bild geschrieben habe.
* Im Fenster: `APPS` als Ordner-Icon, `HELLO.TXT` als Datei-Icon, beide mit
  lesbarem Label in der 6×6-Schrift.

Damit ist **eine durchgehende Kette** belegt: EMMC-Treiber → MBR → FAT16-BPB →
Wurzelverzeichnis-Durchlauf → `desk`-Datenmodell → AES (`svc #2`/200) → VDI
(`svc #2`/115) → Framebuffer. Ein Fehler an irgendeiner Stelle dieser Kette
hätte eine andere Zahl als 24 oder ein anderes Icon ergeben.

Screendumps: `desk_sd.png` (mit Karte) und `desk.png` (ohne Karte) im
Arbeitsverzeichnis des Audits, nicht ins Repo eingecheckt.

**Was der Lauf nicht belegt:** alles Interaktive. QEMU liefert in dieser
Konfiguration keine Maus-/Tastatureingabe an den Desktop, also ist kein
Menü-Klick, kein Doppelklick, kein Drag geprüft.

### 7.2 Menüpunkte: 22 von 31 haben einen Handler

`kernel/src/desk/menu.rs:58-124` definiert 31 `Item`-Varianten.
`kernel/src/desk/main.rs:747-820` verteilt sie.

| Menü | live | ohne Handler |
|---|---|---|
| Desk | About (`main.rs:770-777`, `form_alert` als Platzhalter für den Ressourcen-Dialog) | — |
| File | Open, Close folder, Close window, Cycle, Delete, Select all, Set mask, Search, New folder, Info/Rename | **Execute EmuCON**, **Shutdown** |
| View | alle 8 (Icons/Text, 4 Sortierungen, No sort, Size to fit) | — |
| Options | Set background, Install application, Read `.INF`, Save desktop | **Install icon**, **Install devices**, **Remove icon**, **Set preferences**, **Desktop configuration**, **Change resolution** |

**Wichtig und zu Gunsten des Codes:** die neun Punkte ohne Handler sind in
`men_update` (`kernel/src/desk/menu.rs:665-725`) über den Sammelzweig
`_ => false` **abgeschaltet**, also im Menü ausgegraut. Ein Klick darauf ist
nicht möglich. Das ist die ehrliche Variante — kein stiller No-op. Für eine
Vorführung heißt das: nichts hängt, nichts passiert wortlos.

### 7.3 Shell-Schleife

`kernel/src/shell_loop.rs:72` (`sh_main`) / `:216` (`sh_ldapp`) — die
Nachbildung von EmuTOS' `geminit.c:739`-Schleife. Im Boot-Lauf 2026-09-04
belegt: die Schleife startet die per `#Z` konfigurierte Anwendung
(hier `NEWDESK`, der eingebaute Desktop) und wartet auf ihr Ende.
`kernel/src/lib.rs:391` ist der einzige Aufrufer.

**Damit ist der Weg „TeraDesk statt eingebautem Desktop" mechanisch vorhanden**
(`#Z`-Zeile in `NEWDESK.INF` → `sh_ldapp` startet `DESKTOP.NAP`). Ob er trägt,
ist **nicht belegt** — dafür braucht es den gebauten TeraDesk auf der Karte
und einen Lauf.

### 7.4 Gegenüber EmuTOS `desk/` fehlende Module

| EmuTOS | NewTOS | Stand |
|---|---|---|
| `deskmain.c`, `deskapp.c`, `deskdir.c`, `deskfun.c`, `deskinf.c`, `deskins.c`, `deskact.c`, `deskwin.c`, `icons.c`, `gembind.c` | `main.rs`, `app.rs`, `dir.rs`, `dlg.rs`, `inf.rs`, `insapp.rs`, `act.rs`, `win.rs`, `icons.rs`, `mod.rs` | vorhanden |
| `deskpro.c` (Programmstart / `#Z`) | in `main.rs` (`launch_native`) + `shell_loop.rs` | verteilt vorhanden |
| `deskrez.c` (Auflösungswechsel) | — | **fehlt**; per ADR-017 gehört die Auflösung der `config.txt` |
| `deskobj.c`, `deskfpd.c` (Objekt-/Pfadverwaltung) | in `model.rs`/`dir.rs` aufgegangen | vorhanden (anders geschnitten) |
| `deskrsrc.c`, `desk_rsc.c` (Desktop-Ressource) | — | **fehlt**; NewTOS baut die Bäume im Code |

### 7.5 Desktop in Zahlen

| | Anzahl |
|---|---|
| Menüpunkte gesamt | 31 |
| mit Handler (belegt: Rendering; unbelegt: Auslösung) | **22** |
| ohne Handler, im Menü abgeschaltet | **9** |
| EmuTOS-`desk/`-Module ohne Gegenstück | **2** (`deskrez`, Ressourcen-Datei) |
| Behauptung ohne Deckung | **1** (§1.10) |

---

## 8. SDK und Toolchain

Nicht Teil des TOS-API-Vertrags, aber die Schicht, auf der jede portierte
Anwendung steht — und damit für Oktober entscheidend.

### 8.1 Was gebaut ist

Alle Artefakte tragen das Datum 2026-09-04, die Toolchain funktioniert also
heute:

| Anwendung | Artefakt | Typ |
|---|---|---|
| QED (GEM-Editor) | `sdk/apps/qed/build/QED.NAP` (571 KB) | GEM |
| TeraDesk 4.08 | `sdk/apps/teradesk/build/dist/DESKTOP.NAP` (282 KB) + `desktop.rsc` + `icons.rsc` | GEM |
| CoreMark 1.0 | `sdk/apps/coremark/build/CORE.NCP`, `COREVAL.NCP` | Konsole |
| API-Test | `sdk/apps/apitest/APITEST.NCP` | Konsole |
| GEM-Test | `sdk/apps/gemtest/TESTGEM.NAP` | GEM |
| Chain-Test | `sdk/apps/chaintest/CHAINTST.NAP` | GEM |
| Bibliotheken | `sdk/libc/libc.a` (1,3 MB), `gemlib.a`, `cflib.a` | |

**Belegt:** Bau erfolgreich (Artefakte vorhanden, heute datiert).
**Unbelegt:** dass irgendeines davon **läuft**. Es gibt keinen Lauf-Nachweis
für QED oder TeraDesk in dieser Inventur; QEMU liefert dem Desktop hier keine
Eingabe, also kann kein Doppelklick ausgelöst werden.

### 8.2 `uintptr_t` ist 4 Byte bei 8-Byte-Zeigern — **BELEGT (heute gemessen)**

Ich habe eine Sonde mit **exakt** dem Übersetzungsaufruf des
Anwendungs-Builds (`sdk/apps/build-qed.sh:92-96`) übersetzt:

```
error: static assertion failed due to requirement
       'sizeof(unsigned int) == sizeof(void *)'
note:  expression evaluates to '4 == 8'
```

Ursache: `sdk/libc/mintlib/include/stdint.h:141-144` wählt über `__MSHORT__`;
ohne dieses m68k-Makro fällt `uintptr_t` auf `unsigned int` (4 Byte) und
`intptr_t` auf `int` (4 Byte). `-nostdinc` verdeckt clangs korrekte Definition,
weil `-I$MINT` vor `-isystem $RES` steht.

**Ein Detail, das bisher nirgends steht:** die **Bibliothek** ist davon nicht
betroffen. `sdk/libc/build-libc.sh:102` setzt `-I$TOP/mintlib`, aber die
Header liegen in `$TOP/mintlib/**include**/` — der Pfad greift nicht, und
`libc.a` wird gegen **clangs** `stdint.h` übersetzt (8 Byte). Ich habe das
gemessen: dieselbe Sonde mit den libc-Flags übersetzt **fehlerfrei**.
Anwendung und Bibliothek haben also unterschiedliche `uintptr_t` im selben
Binär.

**Schadenslage, ehrlich eingegrenzt.** Heute ist es **latent, nicht aktiv**:
alle EL0-VAs liegen unter 2^32 (`sdk/libc/crt0.S:10-20`, TPA ab VA 0), die
Kürzung ist also verlustfrei. Die BASEPAGE-Felder sind zudem **absichtlich**
32-bittig (`sdk/libc/mintlib/include/mint/basepage.h:12-24`), sodass das
`(T*)(uintptr_t)`-Muster dort konstruktionsbedingt trägt.

Trotzdem ein echter Defekt, aus drei Gründen:
1. C99 garantiert, dass `uintptr_t` jeden `void*` hält. Korrekt geschriebener
   Code, der sich darauf verlässt, bricht.
2. Er hat schon Arbeit gekostet: `sdk/overlays/coremark/newtos/core_portme.h:54-57`
   musste ausdrücklich `__UINTPTR_TYPE__` statt `uintptr_t` schreiben,
   *„because our mintlib's `<stdint.h>` would give us a 32-bit uintptr_t"*.
3. **Sieben** `.err`-Dateien aus den heutigen Builds enthalten
   `-Wint-to-pointer-cast`, u. a.
   `sdk/apps/teradesk/build/obj/environm.err:1`.

Er wird **aktiv**, sobald einer Anwendung eine Adresse ≥ 4 GiB gegeben wird —
also genau dann, wenn TT-RAM hoch abgebildet oder die 64-Bit-Erweiterung
angegangen wird.

### 8.3 Ein Programm bekommt keine Kommandozeile — **BELEGT (Code)**

Die Kette ist vollständig und an beiden Enden belegt:

1. Der Desktop startet eine `.NTP`-Anwendung per Doppelklick mit **leerem**
   Tail (`kernel/src/desk/main.rs:458-460`) — es gibt keinen
   Parameter-Eingabe-Dialog, den EmuTOS für `.TTP` zeigt.
2. Bei einem Dokument-Assoziationsstart *füllt* der Kernel `p_cmdlin`.
3. `sdk/libc/crt0.S:40-59` verwirft es trotzdem: `mov x0, #1` — `argc` ist
   **hart 1**, `argv` ein statisches `{ "", NULL }`. Der Kommentar sagt den
   Grund: mintlibs `parseargs()` ist m68k-Assembler und wird nicht gelinkt.

Folge: keine portierte Anwendung kann Argumente entgegennehmen. CoreMark
musste deshalb in **zwei** Binärdateien aufgeteilt werden
(`core_portme.h:59-66`).

### 8.4 `.init_array` wird nicht ausgeführt — **BELEGT (Code)**

`sdk/libc/crt0.S:29-59` geht direkt von `__libc_init_heap` zu `main`. Es gibt
keine `.init_array`-Schleife. Jede Funktion mit
`__attribute__((constructor))` läuft nicht. Sichtbar geworden ist es an
mintlibs `_starttime`, weshalb `clock()` seit Boot statt seit Programmstart
zählt.

### 8.5 Der ILP32-Laufzeit-Umschalter steht bewusst

Die sieben `*-ilp32.sh`-Bauskripte und `crt0-ilp32.S` sind entfernt — es gibt
keinen ILP32-Bauweg mehr. Der **Laufzeit**-Umschalter bleibt: 37 Stellen
referenzieren `Abi::Ilp32` in `tos-api/src` und `kernel/src`. Das ist eine
dokumentierte Entscheidung (Substrat für den späteren m68k-Thunk), kein
Versehen — **ehrlich nicht unterstützt**, korrekt gekennzeichnet.

Zu prüfen bleibt: der Zweig wird von **keinem** Test und **keinem** Lauf
erreicht (LP64 ist der einzige Bauweg). Tote, ungetestete Pfade in 17 Dateien
sind eine stehende Wartungslast.

### 8.6 Ein bekanntes Problem, das inzwischen behoben ist

`STATUS.md:370` führt als offen: *„`build-*.sh` verwendet veraltete
Bibliotheksarchive stillschweigend weiter"*. Das stimmt nicht mehr:
`sdk/apps/build-qed.sh:55-66` prüft mit `lib_stale` (`sdk/lib-stale.sh`) jedes
Archiv gegen sein Overlay **und** sein Bauskript und baut es bei Bedarf neu.
Eine weitere Stelle, an der die Dokumentation hinter dem Code liegt.

### 8.7 SDK in Zahlen

| | Anzahl |
|---|---|
| gebaute Anwendungen | **6** |
| davon mit Lauf-Nachweis in dieser Inventur | **0** |
| bestätigte Toolchain-Defekte | **3** (`uintptr_t`, `argc`, `.init_array`) |
| bewusst stehengelassen | **1** (ILP32-Laufzeitzweig) |
| Behauptung ohne Deckung | **1** (§8.6) |

---

## 9. Hardware-Schicht (nicht TOS-API, aber demo-entscheidend)

### 9.1 Speicher: auf einer 4-GB-Pi wird nur die erste Bank verwaltet

Zwei unabhängige Deckel, beide im Code belegt:

1. **Nicht zusammenhängende Bänke werden verworfen.**
   `kernel/src/lib.rs:439-452` nimmt `report.arm[0]` und hängt weitere Bänke
   nur an, wenn `r.base == end`; sonst
   `„ARM bank at … is not contiguous — not managed"`. Auf einer 4-GB-Pi meldet
   die Firmware typisch Bank 0 bis knapp unter `0x4000_0000` (die
   GPU-Reservierung sitzt darunter) und die nächste bei `0x4000_0000` —
   nicht zusammenhängend, also fällt sie weg.
2. **Harte Obergrenzen.** `kernel/src/bios/machine/layout.rs:105`
   `POOL_MAX_BYTES = 1 GiB` (Größe der statischen Frame-Bitmap) und `:120`
   `TOS_MD_MAX_END = 0x7FFF_F000`. Der zweite ist **vertragsbedingt richtig**
   — ein TOS-Memory-Descriptor ist ein vorzeichenbehaftetes LONG und kann
   nichts oberhalb 2 GiB beschreiben. Der erste ist eine Implementierungsgrenze.

Gemessener Boot 2026-09-04 (QEMU `raspi4b`): Firmware meldet **960 MiB**,
ST-RAM-Pool 10 MiB, TT-RAM-Pool **942 MiB**, `OSrsvd` 2 MiB. Unter beiden
Deckeln, also hier unauffällig.

**Einstufung: ehrlich nicht unterstützt, mit Einschränkung.** Der Verzicht
wird im Boot-Log ausdrücklich gemeldet — kein stiller Verlust. Für eine
Vorführung ohne Rolle; für einen 8-GB-Pi relevant.

### 9.2 USB: HID-Routing behandelt Hubs als Mäuse — **BELEGT (Code)**

`kernel/src/drivers/usb/mod.rs:136-141`:

```rust
if c.protocol(di) == 1 { hid::keyboard_report(&report); }
else                   { hid::mouse_report(&report);    }
```

Alles, was nicht Protokoll 1 (Tastatur) ist, wird als **Mausbericht**
interpretiert — auch Protokoll 9 (Hub-Statusänderung) und Protokoll 0 (Slot,
dessen Enumeration scheiterte). Ein Hub-Statusbericht wird damit zu einer
Mausbewegung mit zufälligem Delta und zufälligem Tastenzustand.

Das ist **kein** Beweis für den vom User beobachteten Befund
(„Maus arbeitet nur an einem der vier Ports"), aber ein belegter Defekt
derselben Familie an demselben Pfad. Ich habe **keine** Stelle gefunden, die
Ports beschränkt: `xhci/mod.rs:1138` durchläuft `1..=self.ports` und
rekursiert durch Hubs, `MAX_HID = 8` (`:96`). Auf QEMU nicht untersuchbar —
`raspi4b` modelliert den VL805-PCIe-xHCI nicht.

**Einstufung: verdrahtet, unbelegt; ein zugehöriger Defekt belegt.**
Verifikation nur auf echter Pi 4.

### 9.3 Was der Boot-Lauf 2026-09-04 sonst belegt

`[fb] VideoCore display geometry detected: 640x480` /
`[fb] VideoCore fb up: 640x480 stride 640px` — Mailbox-Framebuffer.
`[emmc] SD card on controller @ 0xffff0000fe300000` — SD-Host-Treiber.
`[mm] MMU reinstalled (high-half via TTBR1 …)` — MMU.
**0 Exceptions** über den ganzen Lauf, sauberer Durchlauf bis zum Desktop.

### 9.4 Unbelegt in dieser Inventur

* Der 200-Hz-Systemtick (`_hz_200` bei `$4BA`) — im Boot-Log nicht sichtbar,
  der Selftest dormant.
* Tastatur- und Mauseingabe im Desktop — QEMU liefert sie in dieser
  Konfiguration nicht.
* Alles auf echter Pi-4-Hardware.

---

## 10. Gesamtbild in Zahlen

### 10.1 Nach Gruppe

| Gruppe | Oberfläche | verdrahtet | **umgesetzt** (belegt) | **verdrahtet, unbelegt** | **ehrlich n. u.** | **fehlt** | **BoD** |
|---|---:|---:|---:|---:|---:|---:|---:|
| GEMDOS | 54 | 52 | 10 | 42 | 1 (+3 `Pexec`-Modi) | 1 | 3 |
| BIOS | 12 | 12 | 0 | 12 | 0 | 0 | 2 |
| XBIOS | ~103 | 33 | 5 | 12 | ~63 (HW) | 11 (nicht HW) | 3 |
| VDI | 67 | 67 | 40 | 19 | 0 | 0 | 3 |
| AES | 79 | 79 | 15 | 64 | 2 | 0 | 2 |
| **API-Summe** | **~315** | **243** | **70** | **149** | **~66** | **12** | **13** |
| Desktop (Menüpunkte) | 31 | 31 | — | 22 | 9 (abgeschaltet) | 2 Module | 1 |
| SDK (Anwendungen) | 6 | 6 gebaut | 0 gelaufen | 6 | 1 | 3 Defekte | 1 |

„VDI umgesetzt = 40" heißt: 40 der 67 Opcodes sind von mindestens einem
Referenzfall abgedeckt **und** alle ihre Fälle sind grün. 8 weitere sind
abgedeckt, haben aber mindestens einen roten Fall; 19 sind von der Suite gar
nicht berührt.

### 10.2 Was die VDI-Suite *nicht* abdeckt

19 der 67 Opcodes haben **keinen** Referenzfall. Darunter die gesamte
**Raster-Gruppe** — `vro_cpyfm` (109), `vrt_cpyfm` (121), `vr_trnfm` (110) —
sowie `vs_color` (14), `v_opnwk` (1), `v_clswk`/`v_clsvwk` (2/101),
`vsf_udpat` (112), `v_locator` (28), `v_string` (31), `vq_key_s` (128),
die fünf `vex_*`-Vektoren (118, 125–127, 134), `vq_mouse` (124),
`vst_load_fonts`/`vst_unload_fonts` (119/120).

Das ist genau der Teil, den eine GEM-Anwendung am härtesten benutzt: jeder
Fensterinhalt, jeder Menü-Save-under, jedes Icon geht durch `vro_cpyfm`.

### 10.3 Die Belegdichte insgesamt

**70 von 243 verdrahteten API-Funktionen (29 %) haben einen laufenden
Nachweis.** Für die übrigen 173 gilt: sie stehen in einer Dispatch-Tabelle,
und mehr weiß ich nicht.

Für den BIOS ist die Zahl **0 von 12**. Das ist die dünnste Stelle der
gesamten Erhebung.

### 10.4 Behauptungen ohne Deckung, gezählt

| Gruppe | Anzahl |
|---|---:|
| übergreifend (Test-Infrastruktur, §1.2, §1.3, §1.7, §1.8) | 4 |
| GEMDOS | 3 |
| BIOS | 2 |
| XBIOS | 3 |
| VDI | 3 |
| AES | 2 |
| Desktop | 1 |
| SDK | 1 |
| **Summe** | **19** |

Davon sind **13 überzeichnend** (die Dokumentation sagt fertig, der Code sagt
etwas anderes) und **6 unterzeichnend** (die Dokumentation nennt etwas offen,
das im Code fertig ist). Beide Richtungen kosten: die eine führt zu falscher
Sicherheit, die andere zu doppelt eingeplanter Arbeit.

---

## 11. Was bis Anfang Oktober realistisch ist

Der Termin ist der 9./10. Oktober, also gut fünf Wochen. Meine Einschätzung,
nach Aufwand geordnet und mit dem Nutzen für eine Vorführung gewichtet.

### 11.1 Sehr kleiner Aufwand, sehr großer Nutzen

| Punkt | Aufwand | Warum es zuerst kommt |
|---|---|---|
| **`cargo xtask test` auf `tos-api` erweitern** (`tools/xtask/src/main.rs:566`) | eine Zeile | Ohne das gibt es keinen Mechanismus, der irgendetwas fängt. `a3563cb` hat zwei Fälle gekippt und niemand hat es gesehen. Solange dieser Befehl null Tests fährt, ist jede Aussage über den Zustand eine Momentaufnahme |
| **Die 15 roten VDI-Fälle adjudizieren** | 1–2 Tage | §5.2 hat die Vorarbeit gemacht: 7 sind Profilunterschied. Sie gehören als solche im Harness markiert, dann ist die Suite wieder ein Netz statt Rauschen |
| **`MAX_VERTICES` 512 → 1024** (`tos-api/src/vdi/primitives.rs:563`) | Stunden | Der einzige eindeutige neue Funktionsdefekt; drei Quellen belegen 1024 |
| **`uintptr_t`/`intptr_t` im SDK korrigieren** | Stunden | Ein Overlay-Header. Beseitigt sieben Warnungen und eine Klasse latenter Fehler |
| **`argc`/`argv` aus `p_cmdlin` + `.init_array` im `crt0`** | ~1 Tag | Macht `.NTP`-Anwendungen und Dokument-Assoziation erst nutzbar; hebt den CoreMark-Zwei-Binärdateien-Behelf auf |

### 11.2 Mittlerer Aufwand, klarer Nutzen

| Punkt | Aufwand | Bemerkung |
|---|---|---|
| **`Setexc(etv_timer)` zustellen** | 1–2 Tage | Der 200-Hz-ISR existiert bereits; es fehlt der Aufruf des installierten Vektors |
| **`Supexec` (XBIOS 0x26)** | 1–2 Tage inkl. ADR | Braucht eine Entscheidung, was „Supervisor-Modus" auf AArch64 heißt. Solange sie fehlt, gibt die Funktion 38 zurück und tut nichts |
| **`Super` (GEMDOS 0x20)** | dito | Dieselbe Entscheidung. Beide zusammen sind ein Arbeitspaket |
| **Referenzfälle für die Raster-Gruppe** (109/110/121) | 2–3 Tage | Schließt die größte Lücke im Netz genau dort, wo GEM-Anwendungen hämmern |
| **Ein Host-Test je BIOS-Funktion** | 2–3 Tage | Von 0 auf 12. Die `fatvol`-Infrastruktur steht schon |

### 11.3 Für den Termin entscheidend, aber nicht durch Code lösbar

**Der größte Risikoposten dieser Inventur ist nicht eine fehlende Funktion,
sondern eine fehlende Messung.** Alles, was ein Zuschauer am 9. Oktober tun
wird — klicken, doppelklicken, ziehen, tippen, ein Menü aufziehen, QED
starten — ist in dieser Erhebung **vollständig unbelegt**. QEMU liefert dem
Desktop in dieser Konfiguration keine Eingabe, und `docs/HW-TEST-LOG.md` endet
im Juni.

Empfehlung, mit Vorrang vor jeder neuen Funktion: **einen Pi-4-Lauf pro Woche,
protokolliert in `HW-TEST-LOG.md`**, mit einer festen Liste der Gesten, die in
der Vorführung vorkommen sollen. Ein Durchlauf, der nichts findet, ist der
Beleg, den diese Inventur für die gesamte Interaktion nicht führen kann.

### 11.4 Bis Oktober **nicht** zu schaffen — und das ist in Ordnung

* Der kooperative Multi-Prozess-Kern und die Desk-Accessories (§6.4). Das
  Substrat steht, der `.ACC`-Lader fehlt ganz. Das ist eine eigene Teilphase,
  keine Restarbeit.
* Die neun abgeschalteten Desktop-Menüpunkte (§7.2). Sie sind ausgegraut,
  also demo-sicher.
* Die Videl-XBIOS-Gruppe 0x58–0x5F (§4.4).
* „EmuTOS-Basis vollständig" im Sinne von Stufe 2a+2b. Nach dem, was ich im
  Code sehe, ist 2a (Bugs) weitgehend abgearbeitet — sechs Stichproben aus dem
  offenen Backlog waren alle implementiert —, aber 2b (fehlende Funktionen)
  hängt am kooperativen Kern.

### 11.5 Der eine Satz

Das System bootet, mountet, liest ein Dateisystem korrekt und rendert einen
vollständigen GEM-Desktop — das habe ich heute gemessen. Was ich **nicht**
messen konnte, ist alles, was ein Mensch damit tut. Die fünf Wochen sind
besser in **Nachweise** investiert als in Funktionen: der Testbefehl, die
Adjudikation der roten Fälle und ein wöchentlicher protokollierter
Hardware-Lauf schließen mehr Risiko als jede einzelne fehlende Funktion.

---

## 12. Methodische Einschränkungen dieser Inventur

Was ich **nicht** getan habe, damit niemand mehr hineinliest, als drinsteht:

* Ich habe die Dispatch-Tabellen vollständig ausgewertet, aber die
  **Implementierungen** nur stichprobenartig gegen EmuTOS gelesen. „Verdrahtet"
  heißt nicht „richtig".
* Ich habe **keinen** Selftest ausgeführt. Sie sind nicht an den Boot
  verdrahtet, und ich habe bewusst keinen Produktcode angefasst, um sie
  einzuhängen — an diesem Baum arbeitete während der Erhebung ein zweiter
  Agent.
* Ich habe **nichts** auf echter Hardware geprüft.
* Drei der roten VDI-Fälle (`vqin_mode` String, `vqt_name` `intout[33]`,
  `v_dspcur` `intin[0]`) konnte ich nicht auflösen. Sie stehen als offen, nicht
  als erklärt.
* Die Zahl „~103 XBIOS-Funktionen" ist aus der EmuTOS-Tabelle gezählt und auf
  zehn genau, nicht auf eins.
* `docs/STATUS.md` habe ich **nicht** korrigiert. Die 19 Stellen ohne Deckung
  stehen in §1 und in den Gruppenabschnitten; was damit geschieht, entscheidet
  der Lead.
