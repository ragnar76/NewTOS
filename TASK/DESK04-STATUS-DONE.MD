# DESK04 — STATUS: DONE (2026-09-24)

**Auftrag:** `Options → Install devices` nachbauen — fuer jedes Laufwerk der
Karte ohne Symbol eines anlegen, bestehende unberuehrt lassen, Persistenz.

Basis: Branch `desk-rebuild`, HEAD `edbb5cc`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, kein `git checkout/restore/clean/stash/reset`.

DESK03 und DESK04 sind **ein** Arbeitspaket. Die tragende Umstellung — die
Desktop-Symboltabelle ist jetzt Zustand (`G.g_ahead`) statt einer Ableitung —
steht in [DESK03-STATUS-DONE.MD](DESK03-STATUS-DONE.MD), einschliesslich des
Defekts, den die Messung dabei fand, und der geaenderten Vorgabeanordnung.
Diese Quittung beschreibt nur, was allein `ins_devices` betrifft.

## Warum der Punkt vorher nichts tun konnte

`app_blddesk` legte bei **jedem** Aufruf ein Symbol je gesetztem Bit von
`dos_sdrv(dos_gdrv())` an. Damit hatte jedes vorhandene Laufwerk immer schon
ein Symbol, und *Install devices* war bauartbedingt ein No-op — nicht
„unimplementiert", sondern ohne moegliche Wirkung. Das ist der Grund, warum
DESK04 von DESK03 abhaengt und nicht umgekehrt.

## Was EmuTOS an dieser Stelle tut

`ins_devices` (`deskins.c:290-333`):

1. `current = dos_sdrv(dos_gdrv());` — die Laufwerkskarte als Bitmaske
   (`:301`);
2. `installed` aus den `a_letter` der `AT_ISDISK`-Knoten von `G.g_ahead`
   (`:306-309`);
3. **erst entfernen**: `installed & ~current` → `remove_drive` (`:314-325`);
4. **dann anlegen**: `current & ~installed` → `install_drive` (`:330-341`);
5. Rueckgabe: ob sich etwas geaendert hat — nur dann ruft `do_optnmenu`
   `app_blddesk()` (`deskmain.c:847-852`).

`install_drive` (`:228-255`) sucht die erste freie Rasterstelle **vor** der
Knotenanlage (`ins_posdisk(0, 0)`, `:231`, damit der neue Knoten sich nicht
selbst sieht), setzt `a_letter = 'A' + drive`, `a_type = AT_ISDISK`,
`a_aicon = (drive > 1) ? IG_HARD : IG_FLOPPY` (`:249`) und `a_dicon = NIL`.

## Was getan wurde

`desk::ins::ins_devices` / `install_drive` / `remove_drive`, in dieser
Reihenfolge und mit dieser Rueckgabe; `ins_posdisk`/`grid_free`
(`deskins.c:110-221`) als Rasterhilfen; der Handler in `do_optnmenu`
(`Item::InstallDevices`), der bei `true` `app_blddesk()` + das
`do_wredraw(DESKWH, &G.g_desk)` faehrt, das EmuTOS ans Ende von
`app_blddesk` haengt (`deskapp.c:1402`), und danach `men_update` — die
Auswahl ist mit den Objekten gestorben.

Die Laufwerkskarte kommt als `dos_sdrv(dos_gdrv())` ueber den GEMDOS-Trap
(DESK10 hat den Trap gebracht, DESK11 den richtigen Aufruf). Der Wert wird
**ausserhalb** des Zustandsschlosses gelesen: er ist ein Trap, kein
Desktopzustand.

`men_update` schaltet den Punkt nie ab: EmuTOS nennt `IDSKITEM` in keiner
`ILL_*`-Tabelle (`deskmain.c:143-160`) und schaltet ihn nirgends von Hand ab.

**Die Beschriftung** eines neu angelegten Symbols ist dieselbe, die der
Startpfad vergibt (`inf::DISK_LABEL` / `FLOPPY_LABEL`) — `install_drive` und
`build_inf` lesen dieselben zwei Konstanten. Dass diese `Hard disk` /
`Floppy disk` lauten und nicht EmuTOS' `DISK C`, ist eine aeltere Abweichung;
sie ist in DESK03s Quittung benannt.

## Pruefung

**Hosttest: keiner moeglich** (dieselbe Begruendung wie in DESK09/DESK10:
`kernel/` traegt `forced-target`).

**`desk::ins::selftest()` — 14 von 14 bestanden**, davon deckt der groessere
Teil genau diesen Punkt ab, und zwar die Faelle, die QEMU **nicht** stellen
kann, weil sich seine Laufwerkskarte nie aendert:

* auf einem vollstaendigen Desktop meldet `ins_devices` **keine** Aenderung
  (der No-op-Fall, der vorher der einzige war);
* nach dem Entfernen **aller** Laufwerkssymbole legt ein einziger Aufruf
  wieder genau eines je Kartenbit an, und jedes benennt ein Laufwerk, das es
  gibt;
* `IG_HARD` gegen `IG_FLOPPY` nach `drive > 1`;
* kein Rasterplatz doppelt belegt (`ins_posdisk` tut, was es soll);
* ein Laufwerk, das aus der Karte **verschwindet** (ein untergeschobenes
  `Z:`), verliert sein Symbol;
* Papierkorb und Drucker ueberstehen den Lauf unberuehrt.

**QEMU raspi4b** (`measurements/measurement-2026-09-24-desk03-desk04/`, 640x480,
Scratch-Kopie mit dem IKBD-Injektor): `C:` auswaehlen → `Options → Remove
desktop icon` (`rmv-C-removed.png`: Symbol weg, Papierkorb und Drucker
unveraendert) → `Options → Install devices` (`rmv-D-reinstalled.png`: Symbol
zurueck). `rmv-D-reinstalled.png` gegen `rmv-A-idle.png`: **0 abweichende
Pixel** (bbox None) — der Punkt stellt den Desktop exakt wieder her.
Null `[EXC]`/`[PANIC]`; `fsck_msdos -n` auf der Partition sauber.

**Bau:** frisch je Board; `--board rpi4` und `--board qemu-virt`: **tos-api 64
/ kernel 94 auf beiden** (Basis 64/94). Clippy tos-api **80**, kernel **125**
auf beiden (Basis 80/125), null Treffer in `kernel/src/desk/`.
`cargo run -p xtask -- test`: **932/0/1** (Basis 932/0/1).

## Artefakte

Siehe [DESK03-STATUS-DONE.MD](DESK03-STATUS-DONE.MD) — dasselbe Aenderungs-
und Belegpaket. Die fuer DESK04 einschlaegigen Stellen:
`kernel/src/desk/ins.rs` (`ins_devices`/`install_drive`/`remove_drive`/
`ins_posdisk`/`grid_free`, `selftest`), `kernel/src/desk/main.rs`
(`do_optnmenu`s `IDSKITEM`-Fall, `rebuild_desktop_icons`),
`docs/api/desktop/deskins.md` (Abschnitt `ins_devices`),
`docs/api/desktop/_hw-test-2026-09-17.md` Punkt **38**, Schritte 0/2/3/9,
`measurements/measurement-2026-09-24-desk03-desk04/` §3 und §8.

## Offen

**Pi-4-Sichtabnahme:** HW-Checkliste Punkt **38**, Schritte 0/3/8 — die
**vier** Laufwerke der Karte (`C:`–`F:`). QEMU zeigt eines, und genau die
Reihenanordnung mehrerer Laufwerke und das „mehrere fehlende in einem Zug"
sind damit am Emulator nicht abschliessend zu sehen. Der `selftest()` prueft
die Logik dahinter unabhaengig von der Laufwerkszahl, aber er zeichnet nicht.

---

## Nachtrag des Leads (gleiche Arbeitseinheit)

Die Symbolbeschriftungen sind nach diesem Bericht auf EmuTOS' Wortlaut
umgestellt worden: `desktop_str_addr(STDISK)` + Laufwerksbuchstabe fuer beide
Laufwerksarten, `TRASH`, `PRINTER` (`desk_rsc.c:2481-2483`, gebaut in
`build_inf` `deskapp.c:723` und `install_drive` `deskins.c:247`). Die oben
protokollierten Laeufe und Screendumps zeigen daher noch `Hard disk` /
`Floppy disk` / `Trash` / `Printer`; die `#M`-Zeile lautet jetzt
`#M 00 00 00 FF C DISK C@ @`. Alles uebrige des Berichts ist unberuehrt.
