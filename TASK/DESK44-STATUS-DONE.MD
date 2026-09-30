# DESK44 — Quittung: `.NTP` ist Konsole mit Parametern

Stand: **fertig** · 2026-09-26 · Baum `desk-rebuild` @ `c2076c0` + unversionierte Änderung
Belege: `measurement-2026-09-26-desk43-u4/` (gemeinsamer Messordner mit U4; DESK44 ist
dort im Abschnitt „DESK44" abgehandelt)

---

## 1. Was der Fehler war

Der Baum behauptete zweierlei über `.NTP`, und `isgraf` hatte sich für die
falsche Seite entschieden:

* `ftype.rs` bildet `.NTP` auf `Flavor::Params` ab, `main::default_flavor`
  bildet `Params` auf `AppFlavor::Ttp` ab, und `inf::default_type_anodes`
  schreibt seit jeher `(b'P', 0x06, -1, "*.NTP", "")` — also `#P`, ein
  **Konsolen**programm mit Parameterzeile.
* `isgraf` gab für `Params` aber `true` zurück, mit dem Kommentar
  *„`.NTP` = `*.GTP` (`#Y`)"* — also GEM.

Der Unterschied ist in EmuTOS **eine** Stelle und genau ein Bit:

```c
/* desk/deskapp.c:310-322, app_parse */
case 'Y':                             /* GEM App needs parms  */
    pa->a_flags = AF_ISPARM;
    FALLTHROUGH;
case 'G':                             /* GEM App no parms     */
    pa->a_type = AT_ISFILE;
    pa->a_flags |= AF_ISCRYS;
    break;
case 'P':                             /* TOS App needs parms  */
    pa->a_flags = AF_ISPARM;
    FALLTHROUGH;
case 'F':                             /* TOS App no parms     */
    pa->a_type = AT_ISFILE;
    break;
```

`#Y *.GTP` trägt `AF_ISPARM | AF_ISCRYS`, `#P *.TTP` nur `AF_ISPARM`
(`desk_inf_data2`, `deskapp.c:181-188`). `isgraf` liest genau `AF_ISCRYS`
(`desksupp.c:876`), gibt es als `isgr` an `shel_write` weiter, die AES merkt es
als `sh_isgem` (`gemshlib.c:173`), und `sh_chgrf` macht daraus den
Bildschirmmodus des nächsten `sh_main`-Durchlaufs (`:513-523`).

**Anwenderentscheidung 2026-09-26:** `.NTP` ist wie `.TTP`. Also ist das
GEM-Verhalten der Fehler.

## 2. Was geändert wurde

| Datei · Funktion | Änderung |
|---|---|
| `kernel/src/desk/main.rs` · `isgraf` | `Flavor::Params` liefert **`false`**; `App` allein liefert `true`. Doc-Block zitiert `deskapp.c:310-322` und nennt die Entscheidung mit Datum |
| `kernel/src/desk/ftype.rs:13` | Tabellenzeile: m68k-Analogon `.TTP` (nicht mehr „`.TTP` / `.GTP`"), Verhalten „VT52 text screen, prompt for a command tail, then run" |
| `kernel/src/desk/ftype.rs` · `Flavor::Params` | Doc: **console** program; warum `.TTP` und nicht `.GTP`, mit `deskapp.c:181-188` + `:310-322` |
| `docs/sdk/01-overview.md:47` | dieselbe Zeile in der SDK-Übersicht; „treated like `.NAP`" war die dritte Kopie des Fehlers und ist weg |

`docs/MEMORY.md` (ADR-018) war bereits vom Anwender nachgezogen und ist **nicht**
angefasst.

## 3. Der Sweep nach weiteren Kopien

`/usr/bin/grep -rn "GTP" --include='*.rs' --include='*.md' …` über den ganzen
Baum ohne `references/`, `target/`, `measurement-*`, `.claude/worktrees/`.
Treffer und Urteil:

| Stelle | Urteil |
|---|---|
| `kernel/src/desk/ftype.rs:13`, `main.rs:1059` (alt) | **die zwei Fehlerkopien** — korrigiert |
| `docs/sdk/01-overview.md:47` | **dritte Kopie** — korrigiert |
| `docs/MEMORY.md:1039` | vom Anwender bereits korrigiert — nicht angefasst |
| `kernel/src/desk/main.rs:1020,1224`, `inf.rs:733`, `ins.rs:606`, `insapp.rs`, `docs/api/desktop/desk{ins,inf,main}.md`, `docs/api/aes/accessories.md:585`, `TASK/DESK33.md:44` | **Aussagen über EmuTOS bzw. über die klassische m68k-Familie** — sachlich richtig, `.GTP` ist dort wirklich gemeint. Unberührt |
| `measurement-*/**.inf`, `hw-round-*/TERADESK.INF` | Messbelege bzw. `#Y 06 FF *.GTP@ @` in erzeugten INF-Dateien — Belege, nicht Behauptungen. Unberührt |

## 4. Was das im Ablauf ändert

`pro_run_native` → `pro::pro_run(isgraf(flavor), …)` → `pro_exec` →
`shel_write(SHW_EXEC, isgr=0, …)`. Der Konsolenarm ist **keine Sonderbehandlung
im Desktop**, sondern der reguläre Weg: `sh_main`s `sh_chgrf` sieht
`sh_isgem == 0` und ruft `sh_toalpha` (`kernel/src/shell_loop.rs:147-155` gegen
`gemshlib.c:513-523`). Es gibt also keine zweite Stelle, die mitgeändert werden
musste.

Unverändert und ausdrücklich benannt: **`opn_appl` ist nicht gebaut**
(`deskinf.c:1253-1271`). Ein `.NTP`-Doppelklick startet weiterhin mit **leerer**
Parameterzeile. Das ist die alte, benannte Lücke (`main.rs` `pro_run_native`,
`TASK/DESK33.md:44`) — dieses Ticket verschiebt nur den Bildschirmmodus, nicht
den Tail-Dialog. Die Abnahmezeile von DESK44 („mit der Parameterzeile aus dem
Tail-Dialog") ist damit **zur Hälfte offen**: der Konsolenarm ja, der Dialog
nein.

## 5. Gegenprobe

| Fall | erwartet | Stand |
|---|---|---|
| `.NAP` | GEM (`isgraf = true`) | unverändert, `Flavor::App => true` |
| `.NCP` | Konsole, kein Dialog | unverändert, `Flavor::Console => false` |
| `.NTP` | **Konsole**, Dialog wäre `opn_appl` | jetzt `false`; Dialog weiterhin nicht gebaut |
| `.NAC` | erreicht `pro_run` nie | unverändert (`pro_run_native` weist ab) |
| `NEWDESK.INF`-Zeile für `*.NTP` | `#P` | war schon `#P` — die beiden Seiten stimmen jetzt überein |

## 6. Was noch geprüft werden muss

**Headless nicht vorführbar** — der Weg beginnt mit einem Doppelklick, und
`raspi4b` hat in diesem Bau keinen USB-Stack (`[pcie] RC not present; USB
disabled`). Was der Tester fahren kann: den Scratch-IKBD-Injektor
(`measurement-2026-09-17-h03/method`, zuletzt `measurement-2026-09-26-desk43-u3`),
ein `.NTP` auf die Karte legen und den Doppelklick schicken. Erwartung:
`[shell] sh_ldapp: starting …, sh_isgem=0` im Seriellprotokoll und ein
**Textbildschirm** statt des Desktops im Screendump.

Auf echter Pi 4 ist es derselbe Weg mit echter Maus.

## 7. Bauen

| | |
|---|---|
| `--board rpi4`, frisch | tos-api 64 / kernel 94 (Basis 64/94, Delta 0), rc 0 |
| `--board qemu-virt`, frisch | tos-api 64 / kernel 94 (Basis 64/94, Delta 0), rc 0 |
| clippy, beide Feature-Sätze | tos-api 80 / kernel 125 (Basis 80/125, Delta 0); 0 Treffer in den berührten Dateien |
| `cargo run -p xtask -- test` | 956 / 0 / 1 |
