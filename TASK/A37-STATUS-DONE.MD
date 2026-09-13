# A37 — Quittung

**Status: BEIDE ANGRIFFSPUNKTE GEBAUT · anwendungsseitige Abnahme offen**
· zweite Haelfte 2026-09-12 · Der Kernel-Angriffspunkt (`fix_tedinfo_std`)
und der Werkzeug-Angriffspunkt (`tools/mkrsc.py`) sind beide gebaut und
hosttest-belegt; die Bauprodukte sind neu erzeugt. **Offen:** die
anwendungsseitige Abnahme (QED zweimal, TeraDesk, Dateiselektor-Dialog,
`aes-r3-wind-state`) liegt beim Tester — hier nicht behauptet. Der Bericht
der ersten Haelfte steht unveraendert unten; die zweite Haelfte ist am
Dateiende angehaengt.

## Quelle

`references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(`git -C references/emutos log -1` bestaetigt), Zielprofil Falcon030/TOS4.

## Der Befund, selbst nachgeprueft

### 1. Der fehlende Nachrechenschritt

`rs_readit` ruft `fix_tedinfo_std` (`aes/gemrslib.c:798`), das nach dem
Relokieren die **Laengenwoerter** aus den Zeichenketten neu setzt
(`:655-669`):

```c
if (fix_ptr(R_TEPTEXT,  ii)) ted->te_txtlen = strlen(ted->te_ptext)  + 1;
if (fix_ptr(R_TEPTMPLT, ii)) ted->te_tmplen = strlen(ted->te_ptmplt) + 1;
```

NewTOS tat das gar nicht: `relocate_blob_core` (`tos-api/src/aes/resource.rs`)
relokierte `te_ptext`/`te_ptmplt`/`te_pvalid` und liess die Laengen aus der
Datei stehen.

### 2. Warum das ein Speicherfehler ist

`tools/mkrsc.py` `intern_str` (`:205-207`) kopiert nur bis zum ersten NUL. Eine
klassische `.rsc` reserviert hinter `te_ptext` dagegen volle `te_txtlen` Bytes.
Selbst nachgemessen an `references/emutos/aes/gem.rsc` `ted[0]`:
`te_ptext = 0x0c8b`, `te_ptmplt = 0x0cb2`, Abstand **39** = `te_txtlen` **39**,
dazwischen 39 Nullbytes (`xxd -s 0x0c80`). Die umgewandelte Fassung hat dort
**1** Byte.

`ins_char` (`tos-api/src/aes/object.rs`, EmuTOS `aes/gemobed.c:117`) schreibt
bis Index `te_txtlen - 1`. `inf_sset` (EmuTOS `util/optimize.c:103`) macht
`strlcpy(te_ptext, pstr, te_txtlen)`. Beide binden sich an `te_txtlen`, nicht
an den Puffer.

Eigene Messung der ausgelieferten Blobs (Parser ueber den NRSC-v5-Kopf):

| Datei | TEDINFOs mit `te_txtlen` > Puffer | schlimmster Fall |
|---|---|---|
| `tos-api/src/aes/gem_fsel.nrsc` | **11 von 13** | `ted[0]`: Puffer 1 B, `te_txtlen` 39 |
| `sdk/apps/teradesk/rsc/en/desktop.rsc` (Quelle) | 6 von 104 | `ted[23]`: 1 B gegen 34 |
| `sdk/apps/qed/src/qed.rsc` (Quelle) | 0 von 118 | — |

Das bestaetigt die Zahlen des Audits. Der Grund fuer QEDs 0: sein
Ressourceneditor legt die Platzhalter in voller Laenge ab
(`ted[8] = "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"`, `te_txtlen` 31) — dann ist
`fix_tedinfo_std` ein No-op. Genau dafuer ist es gedacht.

### 3. Was EmuTOS mit seiner **eigenen** GEM.RSC macht

`fix_tedinfo_std` laeuft **nur** in `rs_readit`. EmuTOS' eingebaute
AES-Ressource kommt ueber `gem_rsc_init` (`aes/gem_rsc.c`) und laeuft **nicht**
durch: `erd` reserviert dort die vollen `te_txtlen` Bytes je TEDINFO
(`tools/erd.c:3007`, `te_ptext_len += get_short(&ted->te_txtlen)`;
`static char rs_te_text[168]`, `{&rs_te_text[0], …, 39, 39}`,
`{&rs_te_text[39], …, 12, 24}`). `te_txtlen` bleibt dort **39**.

Das ist der Grund, warum ich den Nachrechenschritt **nicht** in
`relocate_blob` gelegt habe.

## Was ich geaendert habe

### `tos-api/src/aes/resource.rs`

* Neu `fix_tedinfo_std` (LP64) und `fix_tedinfo_std_ilp32`, dazu die Helfer
  `ted_len` und `has_tedinfo` sowie `TE_TXTLEN_OFF_ILP32` / `TE_TMPLEN_OFF_ILP32`.
* Aufgerufen aus `relocate_and_register_core` direkt nach der Relokation —
  dem `rs_readit`-Pfad. `relocate_blob` (eingebettete GEM.RSC) bleibt
  unberuehrt, entsprechend `gem_rsc_init`.
* Der Lauf geht ueber das **OBJECT**-Feld, nicht ueber das TEDINFO-Feld: nur
  die von einem Objekt referenzierten Substrukturen sind relokiert (getrennter
  Befund **A36**, nicht angefasst) — ein nicht referenziertes TEDINFO traegt
  noch Dateioffsets und darf hier nicht dereferenziert werden.
* `ted_len` begrenzt den `strlen`-Lauf am `total_size` des Blobs. EmuTOS nimmt
  das blanke `strlen`; fuer eine wohlgeformte Ressource sind beide identisch.
  Die Grenze ist vermerkt, kein stiller Unterschied.

### `tos-api/src/aes/object.rs` — untrennbare zweite Haelfte

`ins_char` klemmt jetzt eine negative Einfuegeposition.

`te_txtlen == 1` — was `fix_tedinfo_std` fuer ein Feld mit leerem Text ergibt —
treibt `ob_edit`s Feld-voll-Zweig (`gemobed.c:407-413`) auf `idx == -1`. EmuTOS
bearbeitet eine **Kopie** (`D.g_rawstr`, `:349`) und schreibt das Feld erst
danach mit `strcpy(edblk.te_ptext, D.g_rawstr)` (`:438`); sein `str[-1]` landet
im Scratch und ist unbeobachtbar. NewTOS bearbeitet `te_ptext` an Ort und
Stelle — dieselbe Position haette ein Byte **vor** den Ressourcenpuffer
geschrieben.

Das ist kein theoretischer Fall: der neue Ueberlauftest hat ihn als geaendertes
Blob-Byte **2927** (`te_ptext - 1`) gemeldet, bevor die Klemme drin war. Ohne
sie haette Angriffspunkt (1) einen Schreibzugriff **hinter** dem Puffer gegen
einen **vor** dem Puffer eingetauscht. Das Klemmen gibt EmuTOS' beobachtbares
Ergebnis: Zeichen faellt weg, Feld unveraendert, Cursor bleibt 0.

### `tos-api/tests/aes_rsrc_tedinfo_txtlen.rs` (neu)

Drei Faelle, **alle drei ohne den Fix rot**:

| Test | ohne Fix |
|---|---|
| `tedinfo_lengths_are_recomputed_from_the_strings` | `left: 39  right: 1` (der rote Test des Berichts) |
| `no_tedinfo_claims_more_bytes_than_the_blob_reserves` | `te_txtlen 39 exceeds the 1 bytes the blob reserves` |
| `typing_into_a_loaded_field_touches_nothing_outside_its_text_buffer` | `overwrote its own te_ptmplt template at blob offset 2929` |

Der dritte ist der vom Auftrag verlangte Beleg. Er faehrt den echten
`objc_edit`-Trap (Opcode 46) auf `FSDIRECT` eines geladenen Blobs und
vergleicht **alle** Blob-Bytes ausserhalb des Puffers. Das Fenster ist
absichtlich am **tatsaechlich reservierten** Puffer (`strlen+1`, aus dem Blob
gelesen) verankert und nicht an `te_txtlen` — an `te_txtlen` verankert waere er
im kaputten Zustand leer gewesen (das war meine erste Fassung; sie ging gruen
durch und wurde deshalb verworfen). Die Rohausgabe ohne Fix:

```
left:  [95, 95, 95, …, 95, 0]     die Schablone "______…"
right: [65, 65, 65, …, 65, 0, 0]  37 Byte davon mit 'A' ueberschrieben
```

## Die Entscheidung, und warum

**Gewaehlt: (1), der Kernel.** Grund: `fix_tedinfo_std` ist das, was EmuTOS an
dieser Stelle tut, es ist die Stelle, an der der Projektgrundsatz „angleichen"
greift, und es deckt jede Ressource ab, die durch `rsrc_load` kommt — auch
solche, die wir nicht erzeugt haben.

**(2), das Werkzeug, ist zusaetzlich noetig. Nicht schaedlich — noetig.** Nicht
angefasst, wie aufgetragen. Begruendung in drei Punkten:

1. **`mkrsc.py` verletzt den Formatvertrag der `.rsc`, nicht nur eine
   Konvention.** Klassisch stehen hinter `te_ptext` volle `te_txtlen` Bytes
   (oben an `gem.rsc` nachgemessen), und EmuTOS' eigenes Werkzeug `erd` haelt
   genau das ein (`tools/erd.c:3007`). Angriffspunkt (1) repariert die
   **Folge** im Kernel, nicht die **Ursache** in der Datei.

2. **Anwendungen schreiben an `te_txtlen` vorbei direkt in den Puffer, und
   duerfen das.** TeraDesks `rsc_fixtmplt` (`sdk/apps/teradesk/resource.c:355-356`):

   ```c
   for (txtlen = 0; *tmplt != '\0'; tmplt++)
       if (*tmplt == '_') txtlen++;
   ted->te_txtlen = txtlen + 1;
   ted->te_ptext[txtlen] = 0;        /* <- setzt volle te_txtlen Bytes voraus */
   ```

   Der Kommentar daneben sagt es ausdruecklich: *„we cannot copy string into
   ted->te_pvalid, since that is from resource and could be shortened"*. Diese
   Anwendung laeuft mit einem `strlen+1`-Puffer **auch nach (1)** darueber
   hinaus. Kein Kernelfix kann das abfangen.

3. **Ist nach (1) noch Platz zum Tippen? Nein — und das ist bei EmuTOS genauso.**
   `te_txtlen = strlen+1` heisst fuer ein Feld, dessen Ressourcentext leer ist,
   `te_txtlen = 1`, also **null** tippbare Zeichen. Das ist aber kein
   NewTOS-Artefakt: real TOS/EmuTOS rechnen fuer dieselbe Datei denselben Wert
   aus. Wohlgeformte Ressourcen umgehen es, indem sie den Platzhalter in voller
   Laenge ablegen (QED: 0 von 118 betroffen); TeraDesk umgeht es, indem es
   `te_txtlen` selbst setzt (Punkt 2) **und einen eigenen Ressourcenlader
   mitbringt**, der `fix_tedinfo_std` gar nicht ausfuehrt
   (`sdk/apps/teradesk/rsc_load.c:345-356`: nur `+= (size_t)buf`, keine
   Laengenkorrektur). Der Verlust an Tippkapazitaet ist also **kein** Argument
   gegen (1) — er ist ein Argument dafuer, dass (2) den Puffer in voller Laenge
   anlegen muss, damit derselbe Vertrag gilt wie ueberall sonst.

   Mit **beiden**: Datei reserviert `te_txtlen` Bytes (wie `erd`), Lader rechnet
   nach (wie `rs_readit`) — das ist Punkt fuer Punkt EmuTOS' eigene Anordnung.

**Was (2) kosten wuerde, und warum ich es dem Nutzer vorlege:** geaenderte
Werkzeugausgabe = neu erzeugte Bauprodukte (`tos-api/src/aes/gem_fsel.nrsc`,
`sdk/apps/teradesk/build/dist/*.rsc`, `sdk/apps/qed/build/qed.rsc`). Fuer
`gem_fsel.nrsc` waeren das ~150 Byte mehr; fuer `desktop.rsc` entsprechend. Das
ist ausdruecklich die Entscheidung des Nutzers.

## `fsel::set_ted` — bleibt bewusst stehen

`set_ted` (`tos-api/src/aes/fsel.rs`) biegt `te_ptext` auf Kernelpuffer um und
setzt `te_txtlen = cap`. Das deckt genau die 11 betroffenen TEDINFOs ab
(`FSDIRECT` = `ted[0]`, `FSSELECT` = `ted[1]`, `F1NAME`…`F9NAME` = `ted[3..11]`)
— deshalb faellt der Dateiselektor heute nicht um.

**Ruecknehmen kann ich das nicht, und zwar aus einem anderen Grund als
`te_txtlen`.** `set_ted` tut noch zwei Dinge: es baut `G_FBOXTEXT`/`G_FTEXT` auf
`G_BOXTEXT`/`G_TEXT` zurueck und leert Schablone und Pruefzeichenkette, weil
NewTOS **klare** Namen/Pfade in seinen Puffern haelt und nicht die
schablonen-rohe Form, die `"Selection: ________.___"` erwartet. EmuTOS'
`fs_input` macht das Gegenteil: es liest `ad_fpath = tedinfo->te_ptext`
(`aes/gemfslib.c:682,687`) und benutzt den Blobpuffer weiter — was dort geht,
weil `gem_rsc_init` die vollen 39/12/13 Bytes bereitstellt.

Das ist eine eigene, aeltere Abweichung im Dateiselektor und gehoert nicht zu
A37. Nach (2) waere der Weg frei, sie anzugehen; heute ist sie nicht mehr die
**Absicherung** gegen den Ueberlauf, nur noch eine Darstellungsentscheidung.

## Belege

* `cargo run -p xtask -- test`: **469 → 472 gruen / 0 rot / 1 ignoriert**
  (+3 = die drei neuen Faelle).
* `cargo run -p xtask -- build --board rpi4` und `--board qemu-virt`: beide
  Exit 0. Fuer den rpi4-Lauf vor der Messung wurden `kernel/src/lib.rs` und
  `tos-api/src/lib.rs` angefasst, damit das geteilte Zielverzeichnis nicht
  „up to date" melden konnte.
* Clippy: `tos-api` **81 Warnungen vorher, 81 nachher** (Delta 0); `kernel`
  125 auf beiden Boards (nicht angefasst). Keine Warnung in den neuen
  Funktionen, keine im neuen Testmodul.
* Gegenmessung `measurement-2026-09-12-qemu-r3-a37/` (QEMU `raspi4b`, 640x480,
  `#Z`-Autostart, `aes-r3-wind-state.tsv` 39 Faelle CRC `979FAC60` gegen
  `EMUTOS14-AES-r3.BIN`): **Endzustand haelt** — in den `int_out`-Nutzdaten
  weichen nur 261/280/400/481/494/574/588 ab, 14 `int_out`- und 234
  `global[]`-Zeilen wie vorher. Der Zeilenvergleich der beiden DIFF-Berichte
  laesst ausser `global[11]` (buildabhaengige Kerneladresse) **null**
  Unterschiede:

  ```
  diff a33/DIFF… a37/DIFF… | grep '^[<>]' | grep -vc 'global\[11\]'   ->  0
  ```

  Der Lauf ist ein **No-Regression**-Beleg, kein positiver: der Dumper ruft
  `rsrc_load` nicht, und der Dateiselektor haengt an `relocate_blob`, das
  unberuehrt blieb. Der positive Beleg ist der Hosttest.
* Kein QEMU-/Hatari-Prozess uebrig (`pgrep` nach jedem Lauf leer).

## Was ich nicht angefasst habe

* `tools/mkrsc.py` — Angriffspunkt (2), dem Nutzer vorgelegt (siehe oben).
* `tos-api/src/aes/window.rs`, `docs/api/aes/window-graphics.md` — fremde
  Nachbararbeit.
* `tosdump/cases/`, `tosdump/reference/`, alle bestehenden `measurement-*`.
  Die Falltabelle wurde nur **kopiert** (`C:\AES.TSV` auf dem Kartenabbild).
* **A36** (nur referenzierte Substrukturen relokiert) und **A38** (zu kurz
  gelesenes Blob) — beruehren dieselbe Gegend, sind andere Tickets.
* Kein bestehender Test musste geaendert werden.

## Offen

1. **`tools/mkrsc.py`: volle `te_txtlen` Bytes reservieren** — die Entscheidung
   des Nutzers. Ohne sie bleibt der Ueberlauf fuer Anwendungen offen, die
   direkt in `te_ptext` schreiben (TeraDesk tut das).
2. **`fix_tedinfo_std_ilp32` ist heute unerreichbar und ungetestet.** Genau wie
   sein Zwilling `relocate_blob_core_ilp32`: ILP32-Programme werden seit
   ADR-020-Addendum beim Laden mit `EPLFMT` abgewiesen
   (`tos-api/tests/loader_ilp32_rejected.rs`). Der Zwilling ist mitgezogen,
   damit die beiden Pfade nicht auseinanderlaufen, wenn der m68k-Thunk kommt.
3. **Interaktive Abnahme auf echter Pi 4 steht aus** — Tippen in ein
   Dateiselektor-Feld und in einen TeraDesk-Dialog. Der Hosttest belegt den
   Speicherzugriff, nicht das Bedienerlebnis.

---

# A37, zweite Haelfte — `tools/mkrsc.py` reserviert die deklarierte Laenge

**2026-09-12.** Quelle wie oben: `references/emutos` @
`1a7484aba1bfba23e2c0899e243b6d8177fe41df` (`git -C references/emutos log -1`
selbst bestaetigt).

## Die drei Laengenfelder sind nicht gleich definiert

Selbst nachgemessen, nicht aus `te_txtlen` verallgemeinert.

| Zeiger | Laengenwort | Was es bedeutet | Wer schreibt | Reservierung |
|---|---|---|---|---|
| `te_ptext` | `te_txtlen` | **Pufferkapazitaet** inkl. NUL; der Ressourcentext ist oft kuerzer, oft leer | `ob_edit`/`ins_char`, `inf_sset` — gebunden an `te_txtlen`, **nicht** an die Zeichenkette | **volle `te_txtlen`**, in einem **eigenen** Puffer |
| `te_ptmplt` | `te_tmplen` | **Zeichenkettenlaenge** (*"length of template string"*, `include/obdefs.h:182`) | niemand — nur-lesend (`strcpy(D.g_tmpstr, …)`, `aes/gemobed.c:348`) | `max(deklariert, strlen+1)` = faktisch `strlen+1` |
| `te_pvalid` | **keines** | eine Pruefstelle je editierbarer Position, absichtlich kuerzer | niemand — `ob_edit` dehnt in einen Kratzpuffer (`gemobed.c:351-355`) | `strlen+1`, unveraendert |

Messwerte, die das tragen (eigener Parser ueber den klassischen `.rsc`-Kopf):

* **`gap(te_ptext) == te_txtlen`** in **allen** TEDINFOs von
  `references/emutos/aes/gem.rsc` (13), `sdk/apps/teradesk/desktop.rsc` (104)
  und `sdk/apps/qed/src/qed.rsc` (118) — **0 Ausnahmen**. `gem.rsc` `ted[0]`:
  `te_ptext` 0x0c8b, `te_ptmplt` 0x0cb2, Abstand **39** = `te_txtlen` 39.
* **`te_tmplen == strlen+1`** in denselben 235 TEDINFOs — **0 Ausnahmen**.
  Deshalb kostet das Ehren des deklarierten Werts dort nichts.
* **`gap(te_pvalid) != strlen+1`** in 42 von 104 bzw. 58 von 118 Faellen: es
  gibt dort schlicht keinen Vertrag, den man reservieren koennte.
* `te_ptext`-Offsets werden in keiner der vier gemessenen Dateien von einem
  zweiten TEDINFO, einem `ob_spec`-Etikett, einem `frstr`-Eintrag oder einem
  `ib_ptext` mitbenutzt — die private Reservierung ist fuer unseren Bestand
  also unbeobachtbar und nur die sichere Seite.

EmuTOS' eigenes Werkzeug haelt denselben Vertrag: `tools/erd.c:3006-3007`
(`te_ptext_len += get_short(&ted->te_txtlen)`) reserviert je Feld die vollen
`te_txtlen` Bytes in `rs_te_text[]`, und `gem_rsc_init` rechnet **nicht** nach.

## Was ich geaendert habe

`tools/mkrsc.py`, in **beiden** Konvertern (`convert` und `convert_rshdr`):

* neue Politik-Funktion `ted_reserve(d, coff, declared)` — eine Stelle, an der
  die Begruendung steht, statt zweimal derselbe Ausdruck;
* `intern_str(coff, reserve=0, kind="s")` polstert die Zeichenkette mit NUL auf
  die reservierte Groesse; der Zwischenspeicher ist nach `("s"|"t"|"b", coff,
  size)` getrennt, damit eine Reservierung nie aus einer kuerzeren Kopie
  bedient wird und ein beschreibbarer `te_ptext`-Puffer nie mit einem
  nur-lesbaren Etikett zusammenfaellt;
* Nachbedingung im Werkzeug: keine zwei internierten Daten ueberlappen, und
  jede `te_ptext`-Reservierung deckt ihr `te_txtlen` und liegt im Blob.

**B2 (`RASTER_ALIGN = 2`) bleibt — nachgeprueft.** Die Reservierungsbytes
landen im selben `data`-Abschnitt; `intern_bytes` polstert weiterhin vor jedem
Raster. Gegenprobe mit einer Kratzkopie des Werkzeugs **ohne** diese Polsterung
auf der erweiterten Testvorlage: LP64 `[407, 411, 443]`, ILP32 `[319, 323,
355]`, RSHDR `[383, 387, 419]` — je **3 ungerade Raster**, der alte Fehler
waere also weiterhin aufgefallen. `tools/mkrsc.py` selbst wurde dafuer nicht
angefasst.

## Bauprodukte, mit Herkunft

| Datei | Befehl | vorher | nachher |
|---|---|---|---|
| `tos-api/src/aes/gem_fsel.nrsc` | `cargo run -p xtask -- gen-fsel-rsc` | 4819 B | **4975 B** (+156; 11 von 13 TEDINFOs waren zu kurz) |
| `sdk/apps/teradesk/build/dist/desktop.rsc` | `sdk/apps/build-teradesk.sh` | 39043 B | **39114 B** (+71; 6 von 104) |
| `sdk/apps/teradesk/build/dist/icons.rsc` | dito | 13284 B | **13284 B, bytegleich** (0 TEDINFOs) |
| `sdk/apps/qed/build/qed.rsc` | `python3 tools/mkrsc.py convert sdk/apps/qed/src/qed.rsc …` | 41190 B | **41190 B, bytegleich** (sha `ebfcfdaf…`; 0 von 118 betroffen) |

Dass QED und `icons.rsc` **bitgleich** herauskommen, ist der beste Beleg, dass
eine bereits wohlgeformte Ressource unveraendert durchlaeuft.

## Belege

Test vor Fix, **alle acht vorher rot**:

* `tools/xtask/src/rsc_align_tests.rs` (+6): Reservierung in allen drei
  Ausgabeformaten, private Textpuffer in zweien, und der eingecheckte
  `gem_fsel.nrsc`. Dessen Rohmeldung ohne Fix:
  `tedinfo[0] te_ptext (te_txtlen) declares 39 bytes but the blob reserves
  only 1 behind offset 2928`.
* `tos-api/tests/aes_rsrc_tedinfo_reserve.rs` (neu, 2): auf dem **eingebetteten**
  Pfad (`relocate_blob` = EmuTOS' `gem_rsc_init`, kein Nachrechnen — dort
  entscheidet die Datei allein). `an_empty_selector_field_has_the_typing_room_it_declares`
  ist die verlangte Gegenprobe (`te_txtlen` 39 > 1 ueber leerem Text);
  `typing_a_selector_field_full_stays_inside_its_own_buffer` faehrt den echten
  `objc_edit`-Trap (Opcode 46) auf `FSDIRECT` und vergleicht **alle**
  Blob-Bytes. Rohausgabe ohne Fix:
  `left: [95, 95, …, 0]` (die Schablone `"______…"`) gegen
  `right: [65, 65, …, 0, 0]` — 37 Byte mit `'A'` ueberschrieben.

Weiter:

* `cargo run -p xtask -- test`: **481 → 489 gruen / 0 rot / 1 ignoriert** (+8).
* `cargo run -p xtask -- build --board rpi4` und `--board qemu-virt`, je frisch
  gebaut (beide `lib.rs` vorher angefasst, damit das geteilte Zielverzeichnis
  nicht „up to date" melden kann): beide Exit 0, verschiedene ELF-Hashes.
* Clippy-Delta **0**: `tos-api` 81 Warnungen vorher wie nachher, die neue
  Testdatei erzeugt **keine**; `xtask` 1 bestehende (`main.rs:224`,
  `manual_is_multiple_of`, nicht angefasst), die neuen Testfaelle **keine**.
* Rauchprobe QEMU `raspi4b` headless ohne Karte: `[desk] desktop up (0 drives)`,
  **0** Panik/Ausnahme im seriellen Mitschnitt. `pgrep -f
  'qemu-system-aarch64|Hatari'` danach leer.

## `fsel::set_ted` — kann weiterhin nicht zurueckgebaut werden

Die `te_txtlen`-Begruendung ist mit dieser Aenderung **weg**: `gem_fsel.nrsc`
haelt jetzt 39/12/13 Bytes, genau wie EmuTOS' `gem_rsc_init`. Was bleibt, ist
die aeltere, davon unabhaengige Entscheidung: `set_ted` baut `G_FBOXTEXT`/
`G_FTEXT` auf `G_BOXTEXT`/`G_TEXT` zurueck und leert Schablone und
Pruefzeichenkette, weil NewTOS **klare** Namen/Pfade in seinen Puffern haelt
und nicht die schablonen-rohe Form, die `"Selection: ________.___"` erwartet.
Der Rueckbau ist damit moeglich geworden, aber er ist ein **eigener Auftrag**
(Dateiselektor auf EmuTOS' `fs_input`-Modell umstellen), kein Nebenprodukt
von A37.

## Was ich nicht angefasst habe

* `sdk/apps/qed/`, `sdk/apps/teradesk/`, `sdk/cflib/` — unantastbar.
* `tos-api/src/aes/resource.rs`, `tos-api/src/aes/fsel.rs`,
  `tos-api/src/aes/window.rs`, `tos-api/src/bdos/fs/file.rs`,
  `kernel/src/arch/aarch64/exceptions.rs`, `kernel/src/dbg_qedtrace.rs` —
  fremde bzw. frische Nachbararbeit.
* `tosdump/cases/`, `tosdump/reference/`, alle `measurement-*`.
* Kein bestehender Test musste geaendert werden; die Testvorlage in
  `rsc_align_tests.rs` wurde **erweitert** (zweites TEDINFO), nicht ersetzt.

## Offen

1. **Anwendungsseitige Abnahme (Tester):** QEMU `raspi4b`, `#Z`-Autostart —
   QED zweimal auf demselben Abbild (Klemmbrett, Q07), TeraDesk (Menueleiste,
   Symbole), eingebauter Desktop ohne `#Z` mit Dateiselektor-Dialog (zieht das
   neu erzeugte `gem_fsel.nrsc`). Dazu `aes-r3-wind-state` gegen
   `tosdump/reference/EMUTOS14-AES-r3.BIN`. **Das Kartenabbild muss neu gebaut
   werden** (`tools/make-sd-image.sh`), sonst liegen die alten `.rsc` darauf.
2. **Interaktive Abnahme auf echter Pi 4** — Tippen in ein
   Dateiselektor-Feld und in einen TeraDesk-Dialog.
3. **`fix_tedinfo_std_ilp32` bleibt unerreichbar und ungetestet** (unveraendert
   aus der ersten Haelfte).


---

## Abnahme 2026-09-12 — beide Haelften belegt

Nach der Werkzeughaelfte (volle `te_txtlen`-Reservierung in `tools/mkrsc.py`)
vom Lead selbst gefahren, QEMU `raspi4b`, 640x480, Release-Kernel
`ea06b5989793ad35`, **frisch gebautes Kartenabbild** mit allen neu erzeugten
Bauprodukten.

| Lauf | Ergebnis |
|---|---|
| QED, Lauf 1 (frisches Abbild) | startet, legt `C:\CLIPBRD` an — nachgesehen, Vorbedingung fuer Lauf 2 erfuellt |
| QED, Lauf 2 + 3 (`C:\CLIPBRD` **existiert**) | volle Menueleiste `qed Datei Bearbeiten Fenster Suchen Spezial Optionen Shell`, **keine Alarmbox** — Bildschirmabzug |
| TeraDesk | Menueleiste `Desk File View Window Options`, zwei Disketten, Festplatte, Drucker, Papierkorb — alle Symbole sauber, `desktop.rsc` +71 B |
| Eingebauter Desktop (ohne `#Z`) | `[desk] desktop up (1 drives)`, `C:\`-Fenster mit 24 Eintraegen, 0 Panik/Ausnahmen |

Artefakte: `measurement-2026-09-12-a37-abnahme/`.

**Nicht belegt, ehrlich benannt:** das **Erscheinungsbild** des
Dateiselektor-Dialogs. Er braucht einen Klick, und QEMU `raspi4b` hat kein
Eingabegeraet. Belegt ist, dass das neu erzeugte `gem_fsel.nrsc` (4819 -> 4975 B)
beim AES-Start **geladen und relokiert** wird — ein kaputtes Blob haette den
Bootlauf zum Scheitern gebracht. Das Tippen in ein Selektorfeld und in einen
TeraDesk-Dialog bleibt Hardwareabnahme auf der echten Pi 4.
