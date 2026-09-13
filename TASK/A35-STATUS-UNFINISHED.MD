# A35 — Quittung (2. Fassung)

**Status: WEITERHIN NICHT IMPLEMENTIERT · der erste Blocker ist weg, zwei neue
sind belegt** · 2026-09-13 · Die `AESGLOBAL`-Ablage ist entschieden und
umgesetzt (eigener Arbeitsgang, siehe unten). Das Schreiben der drei Felder
scheitert danach **nicht mehr an der Ablage**, sondern an zwei Adressfragen, die
ich nicht raten darf.

## Was seit der ersten Fassung geschehen ist

Der Anwender hat am 2026-09-13 entschieden: `AESGLOBAL` behaelt die TOS-Ablage
(30 Byte, alle Zeigerfelder 4 Byte), und gemlib lernt sie lesen. Das ist
umgesetzt und belegt:

* `tos-api/src/aes/appl.rs` — `appl_init` legt `ap_private` als **vier** Byte ab
  Byte 6 und fasst `ap_ptree` (Byte 10..13) nicht mehr an. `write_lp64` und
  `write_ilp32` sind dadurch identisch geworden und zu **einem** `write`
  zusammengefallen.
* `sdk/gemlib/overlay/gem.h` (neu) und `sdk/gemlib/overlay/mt_gem.h.in` —
  `_AESappglobal`/`_AESrscfile`/`_AESrscmem` lesen die 4-Byte-Felder an 6/10/14.
* Dauertest `tos-api/tests/aes_global_layout.rs`, Spezifikation
  `docs/api/aes/application.md` §`AESGLOBAL`.

Damit hat jedes Feld seine eigenen Byte. `ap_rsclen` waere ab sofort
schreibbar.

## Warum die drei Felder trotzdem nicht geschrieben werden

### 1. `ap_ptree` hat kein Feld, auf das es zeigen koennte

EmuTOS veroeffentlicht in `ap_ptree` den **relokierten** Baumindex *im Blob*
(`fix_trindex`, `aes/gemrslib.c:595-597`):

```c
ptreebase = (LONG *)get_sub(0, rs_hdr->rsh_trindex, sizeof(LONG));
rs_global->ap_ptree = (OBJECT **)ptreebase;
```

Dort steht danach in Slot `i` die **Adresse** der Wurzel von Baum `i`.

NewTOS' NRSC-Baumindex ist etwas anderes: `[u32]` von **Objektindizes**
(`tools/mkrsc.py:417` packt `(toff - p_obj) // CLS_OBJ`), vier Byte breit in
*beiden* ABI-Varianten, und er wird nicht relokiert — `relocate_and_register_core`
(`tos-api/src/aes/resource.rs`) rechnet die Wurzeladressen beim Laden in die
Kernel-Tabelle `RSC.trees` und laesst das Feld im Blob unberuehrt.

Zwei Dinge stimmen also nicht gleichzeitig: der **Inhalt** (Indizes statt
Adressen) und die **Slotbreite** (4 Byte, waehrend `_AESrscfile[i]` beim
LP64-Aufrufer ein natives 8-Byte-`OBJECT *` ist). Auf `base + off_trindex` zu
zeigen hiesse, der Anwendung ein Indexfeld als Zeigerfeld auszuhaendigen; das
ist schlechter als die 0, die heute dort steht.

Ein treues `ap_ptree` braucht ein **erzeugtes** Zeigerfeld in Aufruferbreite,
das die Anwendung erreichen kann. Wo es liegt (im Blockrest hinter dem Blob? im
Format als v7? in einer eigenen Zuteilung?), wem es gehoert und wann es
freigegeben wird — das ist eine Format- und Eigentumsentscheidung, kein
Feldschreibvorgang. Sie steht mir nicht zu.

### 2. Der Kernelpfad hat keine 32-Bit-Adresse zu melden

`ap_ptree` und `ap_rscmem` sind 4-Byte-TOS-Felder. Der Auftrag fuehrte als
Beleg an, beide zeigten in Anwendungsspeicher (`USER_TPA_TOP` = 4 MiB). Fuer den
**EL0**-Pfad stimmt das: `load_into_user` bildet den Blob im Prozessfenster
unterhalb `USER_TPA_MAX_VA` (32 MiB, `process/mmu.rs`) ab, jede solche Adresse
passt in 32 Bit.

Fuer den **EL1**-Pfad stimmt es nicht. `rsrc_load` relokiert dort an Ort und
Stelle in `RSC_BUF`, eine Kernel-High-Half-Statik. Selbst gemessen:

* Hoststand (`cargo test`, derselbe Pfad, den die drei A35-Tests fahren):
  `rsrc_gaddr(R_TREE,0)` = `0x10444ff48`, `fits_u32` = **false**.
* Ziel: der Kernel liegt laut `kernel/src/arch/aarch64/link-rpi4.ld:24-25` bei
  `VA_OFFSET 0xFFFF000000000000 + 0x80000`; die unteren 32 Bit einer
  `RSC_BUF`-Adresse sind dort `0x0008xxxx` — eine Zahl, die wie eine
  physikalische Adresse aussieht und weder in EL0 noch in EL1 etwas erreicht.

Es gibt also Aufrufer, fuer die das Feld exakt ist, und Aufrufer, fuer die es
keinen darstellbaren Wert gibt. Welche der drei Antworten gilt — abschneiden
(und dokumentieren, dass nur der EL0-Wert etwas bedeutet), 0 lassen (und damit
„nichts geladen" behaupten, obwohl geladen ist), oder den EL1-Pfad selbst so
umbauen, dass der Blob eine 32-Bit-Adresse hat — ist eine Entscheidung mit
beobachtbaren Folgen. Nach `CLAUDE.md` Regel 3 habe ich angehalten.

### Warum nicht wenigstens `ap_rsclen`

`ap_rsclen` ist ein Wort und in beiden Pfaden eindeutig. Es allein zu schreiben
macht den Zustand aber schlechter, nicht besser: EmuTOS liest
`ap_rscmem == NULL` als „nichts geladen" (`rs_free`, `gemrslib.c:685`). Ein
`ap_rsclen = 4975` neben einem `ap_rscmem = 0` behauptet zwei Dinge, die sich
widersprechen. Die drei gehoeren zusammen.

## Was der Anwender entscheiden muss

1. **`ap_ptree`:** wo entsteht das Zeigerfeld in Aufruferbreite, und wer gibt es
   frei? (Vorschlag zur Pruefung, nicht als Entscheidung: im selben Poolblock
   hinter dem Blob, seitengerundet mitzugeteilt, mit dem Block bei `rsrc_free`
   freigegeben. Das braucht keine Formataenderung.)
2. **EL1-Pfad:** abschneiden, 0 lassen, oder den Blob auch dort in ein
   32-Bit-erreichbares Fenster legen?
3. Danach erst sind die drei `#[ignore]` in
   `tos-api/tests/aes_rsrc_global_fields.rs` aufloesbar. Ihre Begruendungstexte
   nennen jetzt diese beiden Gruende statt der Ablage.

Die Frage aus der ersten Fassung — ob `rs_free`/`rs_gaddr`/`rs_saddr` die Werte
wie EmuTOS' `rs_sglobe` (`gemrslib.c:675-679`) **zurueck**lesen sollen — bleibt
davon unberuehrt offen. Sie war ausdruecklich nicht Teil dieses Auftrags.

## Belege

* `cargo run -p xtask -- test`: **504 gruen / 0 rot / 4 ignoriert** (drei davon
  die A35-Tests, der vierte der bestehende Doctest).
* Beide Boards `--release` frisch gebaut, Warnungs- und Clippy-Delta 0.
* `measurement-2026-09-13-aesglobal/` — QED und TeraDesk bytegleich, Gegenmessung
  `aes-m05-addrin` unveraendert.
