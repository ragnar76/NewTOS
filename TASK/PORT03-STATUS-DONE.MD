# PORT03 — STATUS: ERLEDIGT (2026-09-25)

**Auftrag:** cflib auf die gepflegte Fassung bringen, die den PORT03-Defekt
(`make_normalpath` kappt den Pfadtrenner) selbst behoben hat — aber nicht
blind: vorher feststellen, was sich sonst geaendert hat, ob
`expected-failures.txt` noch gilt und ob `overlay/` noch gebraucht wird. Eine
Quelle waehlen und pinnen. QEDs Quelltext bleibt unangetastet.

Basis: Branch `desk-rebuild`, HEAD `feb171d`. Der Aenderungssatz liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen,
keine Pi gefahren, nichts committet, kein
`git checkout/restore/clean/stash/reset` auf dem gemeinsamen Baum; die SD-Karte
unter `/Volumes/BOOT` wurde nicht beruehrt.

Messung: [`measurement-2026-09-25-port03/`](../measurement-2026-09-25-port03/).

---

## 1. Richtigstellung vorweg: die Bibliothek war 26 Monate alt, nicht 21 Jahre

Der eingelagerte `cflib-21.tar.xz` war der Quellschnappschuss des
tho-otto-Spiegels vom **2024-04-30** (byteidentisch, sha256
`7f0d69f4…4062e349`), nicht ein Release von 2004. cflib hat seine
Versionsnummer seit 2004 nie erhoeht — `VERSION`/`ChangeLog` sagen nichts
ueber den Stand der Quellen. Abstand zum geklonten `ce57817` (2026-07-14):
**13 Commits**.

## 2. Was sich davon auf unsere Uebersetzung auswirkt

`SRCFILES`/COBJS unveraendert: **129 .c-Dateien** vorher wie nachher. Vier
davon haben sich geaendert:

| Datei | Commit | Was |
|---|---|---|
| `fsnorm.c` | `2ac3e85` 2024-06-12 | `make_normalpath` laesst den angehaengten `\` stehen — der PORT03-Defekt |
| `fsexists.c` | `af02175` 2024-06-12 + `cdb1fdf` 2024-06-18 | `path_exists`: Schluss-`\` abschneiden, dann `Fsfirst` + `Fgetdta()->dta_attribute` statt `Fattrib` |
| `fexists.c` | `cdb1fdf` 2024-06-18 | `file_exists` ebenso auf `Fsfirst` |
| `fontsel.c` | `399f7a6` 2025-04-02 | Leck auf dem FONTSELECT-Pfad (`vst_unload_fonts` + `v_clsvwk`) |

Alles andere: `Makefile`, `README.md`, sechs CI-Skripte, `.travis*` entfallen
— unbenutzt von `build-cflib.sh`.

`cdb1fdf`/`af02175` sind **Weg 3 des Tickets**, stromaufwaerts bereits getan.
Der neue DTA-Zugriff traegt bei uns, weil mintlibs `_DTA` seit ADR-020 die
gepackte 44-Byte-Fassung mit `_Static_assert`-Offsets ist
(`ostruct.h:85-101`) und `Fgetdta` ueber `__newtos_trap0` ein volles 64-Bit-`x0`
zurueckgibt.

## 3. `expected-failures.txt` gilt unveraendert

`compiled OK=129 skipped=0`, `0 today, 0 recorded, 0 new`. Die vier
geaenderten Dateien uebersetzen warnungsfrei; die einzige Warnung im Lauf ist
die alte Zeichensatzwarnung in `menucre.c:51`.

## 4. `overlay/` wird weiter gebraucht

Beide Anpassungen haengen an `__NEWTOS__` und koennen nicht stromaufwaerts
sein. `inline.rsh` passt unveraendert. `overlay/fontsel.c` ist eine Vollkopie
und wurde **auf die neue Fassung umgesetzt** — sonst haette sie `399f7a6`
zurueckgenommen; `diff -u` gegen `ce57817` zeigt genau unseren einen Block.

## 5. Gewaehlte Quelle

**Commit-Pin `ce57817c3f68c7d346d1081768505e81acaee807` (2026-07-14)**, als
`git archive --prefix=cflib-21/ | xz -9` eingelagert unter
`sdk/cflib/cflib-21-ce57817.tar.xz` (sha256 `8427502d…5683ed8e`).

Kein datierter Tarball, weil es keinen neueren **Quell**tarball gibt: der
Spiegel-Tarball ist der Stand, den wir verlassen, und die neueren
`snapshots/cflib/*.tar.bz2` sind m68k-**Binaer**pakete (`usr/lib/**/libcflib.a`,
null `.c`). Kein „latest", sondern ein Commit — dieselbe Regel wie bei
`references/emutos`/`references/freemint`.

## 6. Aenderungssatz

| Datei | Art | Was |
|---|---|---|
| `sdk/cflib/cflib-21-ce57817.tar.xz` | neu | gepinnte Quelle (`git archive` von `ce57817`) |
| `sdk/cflib/cflib-21.tar.xz` | entfaellt | Spiegelschnappschuss 2024-04-30, abgeloest |
| `sdk/cflib/overlay/fontsel.c` | geaendert | Overlay auf `ce57817`s Fassung umgesetzt |
| `sdk/cflib/build-cflib.sh` | geaendert | Tarballname + Herkunfts-/Pin-Begruendung; Neuextraktion, wenn der Tarball juenger als der Baum ist |
| `sdk/lib-stale.sh` | geaendert | cflibs Tarball wird beobachtet (Glob), mit Begruendung der umgekehrten Altentscheidung |
| `docs/sdk/09-port-material.md` | geaendert | §1/§3: cflib ist nicht mehr eingefroren |
| `docs/sdk/01-overview.md` | geaendert | Tarballname |
| `docs/STATUS.md` | geaendert | Recent-Decisions-Eintrag 2026-09-25 |

Kein Kernel-, kein tos-api-, kein QED-Code.

## 7. Nebenbefund: der Tausch waere sonst still verpufft

`lib-stale.sh` beobachtete die Tarballs bewusst nicht, und `build-cflib.sh`
extrahierte nur bei **fehlendem** Baum. Ein neuer Tarball haette auf jedem
bestehenden Arbeitsbaum nichts bewirkt — ohne eine Zeile Ausgabe. Beides ist
fuer cflib geschlossen (Neuextraktion nach Zeitstempel + Beobachtung des
Tarballs). **mintlib und gemlib haben die Paarung noch nicht** — eigenes
Ticket, siehe Bericht.

## 8. Geprueft

| Pruefung | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **944 passed / 0 failed / 1 ignored** (Basis 944/0/1) |
| `sdk/run-host-tests.sh` | **8 passed, 0 failed, 0 skipped** (Basis 8/0/0) |
| Build `--board rpi4` (nach `cargo clean -p kernel -p tos-api`) | tos-api **64**, kernel **94** Warnungen — Delta 0 |
| Build `--board qemu-virt` (nach eigenem `cargo clean`) | tos-api **64**, kernel **94** — Delta 0 |
| `cargo clippy --features board-rpi4` | tos-api **80**, kernel **125** — Delta 0 |
| `cargo clippy --features board-qemu-virt` | tos-api **80**, kernel **125** — Delta 0 |
| `sdk/cflib/build-cflib.sh` | 129/129, `0 today, 0 recorded, 0 new`, LINK OK |
| `sdk/apps/build-qed.sh` | 37/37, LINK OK, 0 undefined |
| `sdk/apps/build-teradesk.sh` | 48/48, LINK OK, 0 undefined |
| QEMU raspi4b: Datei auf QED gezogen | QED oeffnet sie, Titel **`C:\README.TXT`** (voller Name) |
| QEMU raspi4b: Ctrl-O in QED | cflib-Dateiauswahl „Dateien oeffnen" offen, Verzeichnisse korrekt erkannt |
| QEMU raspi4b: TeraDesk ueber `#Z` | kommt hoch |
| `[EXC]`/`[PANIC]` in drei Laeufen | **0** |
| `fsck_msdos -n`, drei Kartenabbilder | sauber |

## 9. Offen

* Reale Pi-4-Geste (echtes Ziehen mit echter Maus) — wie bei PORT01
  Hardwaresache.
* mintlib/gemlib: gleiche Tarball-Beobachtung, wenn deren Pin bewegt wird.
* T10 (duale Injektor-Bewaffnung) bleibt **offen** — hier nur ueber die
  Scratch-Kopie umgangen, die gemeinsame Harness ist unberuehrt.
