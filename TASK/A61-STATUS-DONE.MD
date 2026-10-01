# A61 — Quittung

**Status: DONE (Hostbeleg; keine Wirkung, keine HW-Abnahme noetig)** ·
2026-09-20 · uncommitted im Baum (Basis `68c84ae`). `TASK/A61.md`s Kopfzeile
bleibt unveraendert.

## Was EmuTOS tut (gepinnte Quelle `1a7484ab`)

| Stelle | Befund |
|---|---|
| `desk/gembind.c:907-915` | Bindung: `SH_DOEX = doex; SH_ISGR = isgr; SH_ISCR = iscr;` → `AES_CTRL_CODE(SHEL_WRITE, 3, 1, 2)` — **drei** Woerter deklariert |
| `include/aesdefs.h:600-602` | `SH_DOEX int_in[0]`, `SH_ISGR int_in[1]`, `SH_ISCR int_in[2]` |
| `aes/gemsuper.c:383-384` | `ret = sh_write(SH_DOEX, SH_ISGR, SH_ISCR, …)` — der Dispatcher reicht alle drei weiter |
| `aes/gemshlib.c:157` | `WORD sh_write(WORD doex, WORD isgem, WORD isover, …)` |
| `aes/gemshlib.c:149` | Kopfkommentar zu `doex = 1` (`SHW_EXEC`): „`isover` ignored, should currently always be 1" |
| `aes/gemshlib.c:168-176` | `case SHW_EXEC`: benutzt `isover` nicht |
| `aes/gemshlib.c:187-191` | `case SHW_RESCHNG`: **`gl_changerez = 1 + isover;`** — der eine Verbraucher |
| `aes/gemshlib.c:140-145`, `:177-186` | `SHW_SHUTDOWN`: Kommentar dokumentiert 0/1/2, der Rumpf liest `isover` nicht |

**Korrektur zum Ticket:** der Satz „benutzt `isover` im ganzen Rumpf nicht"
stimmt am gepinnten Stand **nicht** — `SHW_RESCHNG` benutzt es
(`gl_changerez = 1 + isover`, Zeile 188). Das ist der Aufloesungswechsel,
Inventar-Abweichung **A-5** (eigenes Thema, hier nicht angefasst). Fuer den im
Ticket gemeinten `SHW_EXEC`-Pfad gilt der Satz.

## Kreuzreferenz (nicht Quelle)

Compendium/Profibuch liegen **nicht** als PDF in `references/` (nur
`RP-008248-DS-1-bcm2711-peripherals.pdf`). Vorhanden ist stattdessen Ataris
TOS-4.0-Release-Note im FreeMiNT-Baum,
`references/freemint/doc/programmer/atari/tos4rn.stg` (Zeilen 1187-1195,
1347, 1377-1399): `sh_wiscr` unter `sh_wdoex = 1` = „Request AES for setting
up ARGV style parameter passing in the environment string" (0 = nein, 1 =
ja), angekuendigt ueber `appl_getinfo(10)`.`ap_gout4`. Unter `sh_wdoex = 5`
waehlt es die Aufloesungsfamilie (0 = ST/TT, 1 = Falcon) — deckungsgleich mit
EmuTOS' `SHW_RESCHNG`. **EmuTOS implementiert weder `appl_getinfo` (kein
`APPL_GETINFO`-Arm in `gemsuper.c`, keine 130 in `aesdefs.h`) noch den
ARGV-Aufbau**; damit hat das Wort im `SHW_EXEC`-Pfad keine Wirkung, und wir
erfinden keine. XaAES (`xaaes/src.km/xa_shel.c:390,436`) baut ARGV bei
`wiscr == 1` — FreeMiNT-Weiterentwicklung, nicht Basis.

## Entscheidung

Lesen und ablegen, ohne Wirkung — Treue zur Bindung: `int_in[2]` wird wie die
beiden anderen Woerter durch die A19-Klammer (`min(IN_LEN, I_SIZE)`)
gelesen und im `SHW_EXEC`-Pfad im Shell-Zustand abgelegt. Das Feld ist keins
von EmuTOS' `SHELL` (dort ist `isover` nur Parameter); es ist unbeobachtbar
und existiert, damit der Test die Ankunft belegen kann. `SHW_RESCHNG`
bleibt wie bisher (A-5).

## Was gebaut wurde

| Teil | Datei / Funktion | Stand |
|---|---|---|
| Fix | `tos-api/src/aes/shell.rs` `shel_write`: `let isover = pb.intin(2);` — im `SHW_EXEC`-Arm `*S.sh_isover.get() = isover;` | fertig |
| Zustand | `State::sh_isover: UnsafeCell<i16>` (Init 0), kommentiert als Nicht-`SHELL`-Feld ohne Verbraucher | fertig |
| Getter | `pub fn sh_isover() -> i16` — Testhaken, dokumentiert. **`pub`, nicht `pub(crate)`**: ein Integrationstest unter `tos-api/tests/` ist ein fremdes Crate, `pub(crate)` waere von dort unerreichbar; Muster der bestehenden Getter (`sh_nextapp`, `sh_isgem`, `sh_doexec`) | fertig |
| Test | `tos-api/tests/aes_shel_write_iscr.rs` (neu, Referenz-Harness `aes_reference/harness.rs`, echter Dispatcherweg mit `control[IN_LEN] = 3`): (1) `isover` 1 → 1, 0 → 0, 7 → 7 (Wert folgt dem Wort); (2) `IN_LEN = 2` mit vergiftetem `int_in[2]` → Manager liest 0 (A19-Klammer, `gemsuper.c:431-434`) | 2/2 |
| Spec | `docs/api/aes/scrap-shell.md` § `shel_write`: Slot-Tabelle `int_in[2]` praezisiert (fuer jeden Modus gelesen, nur `SHW_RESCHNG` verbraucht, `SHW_EXEC` ignoriert, `SHW_SHUTDOWN` liest nicht); Verhaltensabsatz um TOS-4-Kreuzreferenz (ARGV) und NewTOS-Verhalten (A61) ergaenzt | fertig |

## Geprueft (Belege in `measurements/measurement-2026-09-20-a49-a61/`)

* **Rot vor / gruen nach** (`host-tests/a61-red-before.txt`,
  `a61-green-after.txt`): mit Feld+Getter, aber ohne das Lesen: beide Faelle
  `left: 0, right: 1`; nach dem Lesen 2/2.
* `cargo run -p xtask -- test`: **653 / 0 / 1** (Baseline 648 + 2 A61 + 3 A49).
* Boards rpi4 / qemu-virt (je nach `cargo clean -p kernel -p tos-api`):
  rustc 64/94, Clippy 81/125 — Delta 0; Warnungsarten in `shell.rs` identisch
  zur Vorquittung; Testdatei clippy-sauber (`build/clippy-tests.txt`).
* Kein QEMU noetig: der Wert hat keine Wirkung.

## Offen

- Nichts fuer A61. **A-5** (`SHW_RESCHNG` → `gl_changerez = 1 + isover`,
  Aufloesungswechsel) ist ein eigenes, offenes Thema und wurde hier bewusst
  nicht beruehrt.
- Nicht committet; `docs/STATUS.md` nicht angefasst.
