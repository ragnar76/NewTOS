# measurement-2026-09-26-desk43-u4

Belege zu **DESK43 / U4** — „Der Kernel braucht `desk::` nicht mehr"
(`docs/analysis/desktop-userspace-plan-v2.md` §5 U4) und zu **DESK44**
(`.NTP` ist Konsole mit Parametern).

Baum: `desk-rebuild` @ `c2076c0` + die U4/DESK44-Änderung (unversioniert im
Baum, `method/product-u4.diff` + `method/new-file-xbiosbind.rs.txt`).
EmuTOS-Referenz: `references/emutos`.
Alles headless in QEMU `raspi4b` bzw. `qemu-virt`.
**Keine echte Pi 4, nichts geflasht, `/Volumes/BOOT` nicht angefasst.**

```
serial/       die Seriellprotokolle
screendumps/  der QMP-Screendump des Desktops (rpi4)
analysis/     die beiden Befunde + der Beweis-grep
method/       Produktdiff, die neue Datei, das Startskript, das gefilte Ticket
build/        Bauen, clippy, Hosttests, Driftprobe
```

Kein `*.img` beigelegt (Auflage). Das Abbild entsteht reproduzierbar aus
`tools/make-sd-image.sh <img> 64` — ohne jeden Handgriff danach; die Läufe
benutzen genau dessen Standardinhalt (`\AUTO\HELLO.NCP` + `\AUTO\READ.ME`,
`C:\ACC\SYSACC.NAC` + `C:\SYSACC2.NAC`).

## Die Zahlen

| | |
|---|---|
| **Driftprobe** | **32** (A 20 · B 8 · C 4) — vorher 38 (A 20 · B 12 · C 6) |
| Bauen, beide Boards, je frisch | tos-api **64** / kernel **94** Warnungen (Basis 64/94, Delta 0), rc 0 |
| clippy, beide Feature-Sätze | tos-api **80** / kernel **125** (Basis 80/125, Delta 0); **null** Treffer in `desk/`, `launch.rs`, `accs.rs`, `initinfo.rs` |
| `cargo run -p xtask -- test` | **956 / 0 / 1** (Basis 956/0/1) — dreimal wiederholt, siehe „Der Flake" |
| `EXC`/`PANIC`/`panic` in den Produktläufen | **0** |

### Die Driftzahl, aufgeschlüsselt

| Gruppe | 38 | 32 | Differenz |
|---|---:|---:|---|
| A `tos_api::` in `desk/` | 20 | 20 | unverändert (U5/U7) |
| B `crate::` in `desk/` | 12 | 8 | −1 `crate::keymap`, −3 von 4 `crate::initinfo` |
| C Kernel → `desk` | 6 | 4 | −2: `launch.rs` und `accs.rs` |

**Gruppe C ist damit genau die Liste, die U4 als Abnahme nennt:**
`pub mod desk;` (`lib.rs:62`), `desk::reset()` (`lib.rs:475`) und der
DESKTOP-Arm (`shell_loop.rs:291-292`, zwei Zeilen). Der Beweis-grep steht in
`analysis/proof-grep-crate-desk.txt`; seine drei weiteren Treffer
(`shell_loop.rs:422`, `exceptions.rs:344-345`, `dbg_qedtrace.rs:25-26`) sind
Fließtext in Doc-Kommentaren, keine Pfade — deshalb zählt die Probe sie nicht.

**Nicht erreicht: 26.** Zwei der vier B-Posten stehen noch, beide mit
gemessener Begründung, beide im Bericht benannt:

* **5× `crate::kprintln`** — der Plan sagt „→ `Cconws`". Das geht nicht:
  `Cconws` liest seinen String mit dem *unprivilegierten* `ldtrb`
  (`tos-api/src/gemdos/console.rs:527`), also faultet jedes Kernel-VA-Literal;
  und die Konsole schreibt zusätzlich in den Framebuffer
  (`host_env.rs:58-61` → `fbcon::write_byte` → `vdi::alpha_conout`), malte also
  Zeichen auf den laufenden Desktop. EmuTOS' `kprintf` geht auf das
  *Debug*-Gerät, `bconout1(1, c)` (`bios/kprint.c:91-99`) — und unser
  `Bconout(1)` ist ein abwesendes Gerät, das das Byte verwirft
  (`tos-api/src/bios/chardev.rs:176-186`). Der Ersatz muss vor der Entfernung
  gebaut werden (Plan §6.3).
* **1× `crate::initinfo::machine_name()`** — die Zeile *Machine* der
  About-Box. `_MCH` ist auf **beiden** Boards derselbe feste NewTOS-Marker
  `0x0005_0000` (`tos-api/src/cookie.rs:143`,
  `docs/api/bios/_cookie-jar.md:136`), kann „Raspberry Pi 4 (BCM2711)" also
  nicht von „QEMU aarch64-virt" unterscheiden. Die Zeile ist zudem eine
  NewTOS-Zutat — EmuTOS' `ADDINFO` hat gar keine (`desk_rsc.c:772-873`). Sie zu
  streichen wäre kostenlos und EmuTOS-näher, ist aber eine sichtbare Änderung
  an einer bewussten Eigenzutat: Anwenderentscheidung, nicht die dieser
  Einheit.

Die verbliebenen 2× `crate::sync::SpinMutex` sind ausdrücklich U8s Posten
(Plan §5, U8: „`SpinMutex` → `RefCell`/`Cell`").

## Die Läufe

| Lauf | Kernel | Abbild | Zweck |
|---|---|---|---|
| `boot-rpi4-u4` | U4-Baum, `--board rpi4`, frisch | make-sd-image, Standard | `\AUTO\`-Lauf + `.NAC`-Erkennung + Desktop; Screendump `screendumps/boot-u4-rpi4.png` |
| `boot-qemu-virt-u4` | U4-Baum, `--board qemu-virt`, frisch | — | zweites Board bootet bis `desktop up` |
| `keytbl-probe-truncated` | U4 + Messsonde, **vor** dem Breitenfix | wie oben | der Data Abort, der die `Long`-Verengung belegt |
| `keytbl-probe-fixed` | U4 + Messsonde, **nach** dem Fix | wie oben | Kernel-VA + `shift[0x1E]='A'`, wertgleich zum alten `keymap::to_ascii` |

Die Messsonde lag vorübergehend in `desk::run()` und ist von Hand entfernt;
`grep -rn 'u4probe' kernel/src tos-api/src` ist leer. Sie war **kein**
Produktcode und ist deshalb nicht im Diff.

## Die beiden Befunde

* `analysis/keytbl-truncation.txt` — `tos_types::Long` ist `i32`; eine
  zeigerliefernde XBIOS-Funktion darf nicht durch ihn zurückgegeben werden.
  Gilt latent auch für `dosbind::dos_trap` (heute ruft der Desktop keine
  zeigerliefernde GEMDOS-Funktion).
* `analysis/auto-and-acc-pattern.txt` — der Endungstest sitzt jetzt im
  Suchmuster, wie bei EmuTOS; der Bootlauf zeigt, dass `READ.ME` weiterhin
  durchfällt und beide Accessory-Fundorte weiterhin greifen.

## Der Flake

`build/xtask-test-flake.log`: ein Lauf von `cargo run -p xtask -- test` ist in
`tos-api/tests/sched_pexec_parent_suspend.rs:146`
(`the_childs_pterm_puts_the_parent_back_exactly_as_it_was`) gescheitert —
*„the round-robin sees the parent again, left: 1, right: 2"*. Danach: derselbe
Test **5/5** einzeln grün, die ganze Suite **3/3** grün mit 956/0/1
(`build/xtask-test.log`). `tos-api` ist von dieser Einheit **nicht angefasst**
(`method/product-u4.diff.stat`), der Fehler kann also nicht von ihr kommen.
Als eigener, kleiner Befund gemeldet, nicht behoben.

## DESK44

Im selben Änderungssatz, aber sachlich getrennt (eigene Quittung
`TASK/DESK44-STATUS-DONE.MD`): `isgraf` gibt für `.NTP` jetzt `false`
zurück. `.TTP` ist `#P` = `AF_ISPARM`, `.GTP` ist `#Y` = `AF_ISPARM|AF_ISCRYS`
— `app_parse`s Fallthrough `case 'Y' → case 'G'` bzw. `case 'P' → case 'F'`
(`deskapp.c:310-322`) ist der ganze Unterschied. Headless nicht vorführbar: der
Weg beginnt mit einem Doppelklick. Der Beleg ist die Quellzeile plus die
Gegenprobe, dass `inf::default_type_anodes` schon immer `#P *.NTP` schrieb.
