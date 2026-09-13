# A35 — Quittung

**Status: NICHT IMPLEMENTIERT · Befund bestaetigt · blockiert an einer
ABI-Entscheidung, die mir nicht zusteht** · 2026-09-13 · Der Defekt ist
nachgeprueft und mit roten Tests festgehalten. Das vom Auftrag als
„unstrittig" bezeichnete **Schreiben** der drei Felder ist es fuer einen
LP64-Aufrufer nicht: die Ablage, die gemlib dafuer vorgibt, hat physisch keinen
Platz. Ich habe nicht geraten.

## Quelle

`references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`, Zielprofil
Falcon030/TOS4. Befund **B3** des Audits vom 2026-09-11.

## Der Befund — bestaetigt

`rs_readit` belegt zwei der Felder, sobald die Datei im Speicher steht
(`aes/gemrslib.c:785-788`):

```c
rs_global = pglobal;
rs_global->ap_rscmem = rs_hdr;
rs_global->ap_rsclen = rslsize;
```

und `fix_trindex` das dritte (`:595-597`):

```c
ptreebase = (LONG *)get_sub(0, rs_hdr->rsh_trindex, sizeof(LONG));
rs_global->ap_ptree = (OBJECT **)ptreebase;
```

NewTOS' `rsrc_load` fasst `pb.global_ptr()` nicht an; der einzige Aufrufer im
ganzen Baum ist `appl_init` (`tos-api/src/aes/appl.rs`). Selbst gemessen, mit
dem echten Dispatcher gegen ein FAT16-Abbild im Hostspeicher:

```
---- rsrc_load_reports_ap_rsclen stdout ----
assertion `left == right` failed: ap_rsclen (global[9]) must be the
resource's declared size
  left: 0
 right: 4975

---- rsrc_load_reports_ap_rscmem stdout ----
assertion `left != right` failed: ap_rscmem (global[7]) is still 0 after rsrc_load

---- rsrc_load_reports_ap_ptree stdout ----
assertion `left != right` failed: ap_ptree (global[5]) is still 0 after rsrc_load
```

Der Befund ist also echt und reproduzierbar.

## Warum ich trotzdem nicht geschrieben habe

Der Auftrag nennt das Schreiben unstrittig. Fuer die m68k-/ILP32-Ablage stimmt
das. Fuer **LP64** — die einzige ABI, in der heute ueberhaupt ein Programm
laeuft (`loader.rs:413` weist jedes ELFCLASS32-Abbild ab) — nicht.

gemlibs Zugriffsmakros haben **keine** `__LP64__`-Fallunterscheidung
(`sdk/gemlib/gemlib-0.44.0/gem.h:41,49,57,64`; `aes_global[]` ist
`short aes_global[AES_GLOBMAX]` mit `AES_GLOBMAX 16`, `mt_gem.h:2502`,
`globals.c:40` — 32 Byte). Ein LP64-Aufrufer liest damit:

| Feld | gemlib-Leser | gelesene Bytes |
|---|---|---|
| `ap_private` | `*(long*)&aes_global[3]` | **6..13** |
| `ap_ptree` | `*(OBJECT***)&aes_global[5]` | **10..17** |
| `ap_rscmem` | `*(void**)&aes_global[7]` | **14..21** |
| `ap_rsclen` | `aes_global[9]` | 18..19 |
| `ap_planes` | `aes_global[10]` | 20..21 |

Drei Zeigerfelder zu je 8 Byte sollen in die **14 Byte** zwischen dem Beginn von
`ap_private` und `ap_planes` passen. Sie ueberlappen einander paarweise:

* `ap_ptree` (10..17) ueberschreibt die obere Haelfte von `ap_private` (6..13) —
  dem Feld, das `appl_init` mit `rs_trees[DESKTOP][ROOT].ob_spec` belegt und das
  A06/A25 gemessen haben.
* `ap_rscmem` (14..21) ueberschreibt die obere Haelfte von `ap_ptree` **und**
  `ap_rsclen` **und** `ap_planes` (`gemsuper.c:97`, von A08 gemessen).

Es gibt **keine** Bytebelegung, bei der alle drei plus `ap_private` und
`ap_planes` gleichzeitig stimmen. Auch die m68k-Belegung (4/4/2 Byte an 10/14/18)
hilft nicht: sie laesst zwar alle Felder nebeneinander liegen, aber der
LP64-Leser holt 8 Byte und bekommt `ptree_low32 | rscmem_low32 << 32` — und
`ap_private` wuerde dabei mitzerstoert.

Der Vollstaendigkeit halber, weil es die Dringlichkeit relativiert: **heute
liest ein LP64-`_AESrscmem` bereits Muell** — Byte 20..21 tragen `ap_planes`,
also liefert der 8-Byte-Zugriff `planes << 48`. Die Ablage ist schon jetzt
nicht tragfaehig; mein Nichtstun verschlechtert nichts, mein Raten haette
verschlechtert.

Nach Regel 3 (`CLAUDE.md`: „Der m68k-ABI-Bezug eines Parameters" ist
ausdruecklich ein STOP-Grund) habe ich deshalb angehalten.

## Was EmuTOS mit den zurueckgelesenen Werten tut — Beleg je Aufrufer

`rs_sglobe` (`gemrslib.c:675-679`) setzt bei **jedem** Aufruf
`rs_hdr = pglobal->ap_rscmem`. Was danach damit geschieht:

| Aufrufer | Datei:Zeile | was mit dem Wert aus dem `AESGLOBAL` geschieht |
|---|---|---|
| `rs_free` | `:683-708` | `if (rs_hdr)` → `free_cicon_buffers(rs_hdr)` und `dos_free(rs_global->ap_rscmem)` — **der Kernel gibt einen Zeiger frei, den die Anwendung im Wort stehen hat** |
| `rs_gaddr` | `:711-721` | `*rsaddr = get_addr(rtype, rindex)`; `get_addr` rechnet aus `rs_hdr` und liefert fuer `R_TREE` `rs_global->ap_ptree[rsindex]` (`:123`) — **Dereferenzierung eines app-gelieferten Feldes** |
| `rs_saddr` | `:727-742` | `psubstruct = get_addr(...)`, dann `*psubstruct = rsaddr` — **Schreibzugriff durch eine aus dem App-Wort abgeleitete Adresse** |
| (dazu `rs_fixit`) | `:816-819` | `fix_objects()` laeuft ueber `rs_hdr` und schreibt jedes `ob_spec` |

## Die zwei benannten Moeglichkeiten, mit Kosten

Beide setzen voraus, dass die LP64-Ablage vorher entschieden ist (siehe unten).

### (A) Schreiben **und** ruecklesen — EmuTOS 1:1

`rsrc_load` belegt die drei Felder; `rsrc_free`/`rsrc_gaddr`/`rsrc_saddr` holen
den Deskriptor wie `rs_sglobe` aus dem uebergebenen `pglobal` zurueck.

* **Gewinn:** exakte Angleichung. Und — zwei offene Befunde faellt es als
  Nebenprodukt: **B9** (ein einziger `static RSC` fuer alle Klienten; mit
  Rueckleseweg haelt jede Anwendung ihren eigenen Deskriptor, ein zweiter
  `rsrc_load` verdraengt den ersten nicht mehr) und die `MAX_TREES = 64`-Kappe
  aus B12 (der Baumindex des Blobs waere dann massgeblich, nicht die
  Kernel-Tabelle).
* **Kosten:** der Kernel folgt bei jedem `rsrc_gaddr`/`rsrc_saddr`/`rsrc_free`
  einem Zeiger aus einem Anwendungswort — einschliesslich `Mfree` darauf und
  eines **Schreibzugriffs** hindurch (`rs_saddr`). EmuTOS hatte keinen
  Speicherschutz; wir haben einen, und der EL0/EL1-Schnitt ist das eine, was
  NewTOS ausdruecklich besser macht. Eine Pruefung („liegt der Zeiger im
  RSC-Fenster des aufrufenden Prozesses?") waere selbst eine Abweichung — EmuTOS
  prueft nichts — aber ohne sie ist es ein Loch, das eine fehlerhafte Anwendung
  in einen Kernel-Schreibzugriff verwandelt.
* **Aufwand:** mittel. `get_addr` muesste seinen Deskriptor je Aufruf aus dem
  `pglobal` rekonstruieren statt aus `RSC`; die Zugriffs-/Ziel-Adresstrennung
  (Kernelalias gegen User-VA) muesste daran mitwandern.

### (B) Schreiben, aber die Kernel-Statics bleiben die Wahrheit

`rsrc_load` belegt die drei Felder als **Bericht an die Anwendung**;
`rsrc_free`/`rsrc_gaddr`/`rsrc_saddr` arbeiten weiter mit `RSC`.

* **Gewinn:** der belegte Fehlerfall aus A35 ist weg (TeraDesk liest die Felder,
  `icon.c:2692,2707-2708`). Kein Kernel-Zugriff auf ein Anwendungswort.
* **Kosten:** beobachtbare Abweichung, sobald eine Anwendung das Feld
  **setzt** und danach den AES fragt. Genau das tut TeraDesks eigener Lader
  (`rsc_load.c:417-423`: `*rscfile = _rs_trindex; *rscmem = buf;`) — er ruft
  danach allerdings seinen eigenen `xrsrc_gaddr` (`:504-508`), also faellt es
  bei TeraDesk nicht auf. Eine Anwendung, die `_AESrscmem` setzt und den
  **AES**-`rsrc_gaddr` ruft, bekommt bei EmuTOS ihre Ressource und bei uns die
  zuletzt geladene.
* **Kosten:** B9 und die `MAX_TREES`-Kappe bleiben unveraendert offen.
* **Aufwand:** klein — drei Schreibvorgaenge in `rsrc_load`, ein Nullen in
  `rsrc_free`.

### (C) Die Vorbedingung fuer beide: die LP64-`AESGLOBAL`-Ablage

Ohne Entscheidung ist weder (A) noch (B) ausfuehrbar. Der Weg, der zur
Projektlinie passt („Port: den Werkzeugkasten reparieren, nicht die
Anwendung"), ist `sdk/gemlib/gemlib-0.44.0/gem.h` + `AES_GLOBMAX` fuer LP64 zu
retargeten, sodass jedes Feld seinen eigenen 8-Byte-Platz bekommt, und die
Kernelseite (`AesGlobalVals::write_lp64`) auf dieselbe Ablage zu legen.

* **Kosten:** alle portierten Programme neu bauen (QED, TeraDesk); die
  Referenztabelle des AES-Dumpers fuer `global[]` verschiebt sich; die m68k-/
  ILP32-Ablage bleibt die alte, der Schreiber braucht also beide — die Form
  dafuer steht schon (`write_lp64` / `write_ilp32`).
* **Beruehrt** ausserdem `ap_3resv`, das heute als 4-Byte-Feld an Byte 22
  abgeschnitten wird, und `ap_private`, dessen 8-Byte-Lage an Byte 6 der Grund
  fuer die erste Ueberlappung ist.

Meine Empfehlung, ohne sie zu treffen: **(C), dann (B)**. (A) erst, wenn der
Speicherschutz die Rueckleseadresse pruefen darf — das ist eine eigene
Entscheidung und beruehrt ADR-021 (`usermem`) und das FreeMiNT-Schutzmodell.

## Artefakte

`tos-api/tests/aes_rsrc_global_fields.rs` (neu): baut ein FAT16-Abbild im
Hostspeicher mit `C:\TEST.RSC` und faehrt `rsrc_load` durch den echten
AES-Dispatcher.

* drei Tests fuer das Soll (`ap_rscmem`, `ap_rsclen`, `ap_ptree`) — **rot**,
  mit `#[ignore]` und der Blockadebegruendung im Attributtext abgelegt, damit
  die Suite nicht rot ist und der Grund nicht verlorengeht (`CLAUDE.md`:
  „Never commit failing tests without `#[ignore]` + reason");
* ein **laufender** Test `rsrc_load_leaves_the_three_fields_zero_today`, der den
  heutigen Zustand festhaelt, damit niemand ihn versehentlich halb aendert;
* die Ueberlappungstabelle steht im Modulkopf der Testdatei und im Modulkopf
  von `tos-api/src/aes/resource.rs`.

`docs/api/aes/resource-fileselector.md`: der `rsrc_load (110)`-Abschnitt nennt
die Luecke jetzt ausdruecklich als **Not yet done (A35)** mit dem Grund, statt
das Soll unkommentiert stehen zu lassen.

## Belege

* Suite `cargo run -p xtask -- test`: **499 gruen / 0 rot / 4 ignoriert**
  (3 der Ignorierten sind die A35-Blocker, der vierte ist der bestehende
  Doctest).
* Beide Boards bauen (Zahlen in `TASK/A36-STATUS-DONE.MD`).
* Nichts an `tos-api/src/aes/resource.rs`s `rsrc_load` und nichts an
  `tos-api/src/aes/appl.rs` wurde fuer A35 veraendert.

## Offen — die Entscheidung des Anwenders

1. **(C):** wird die LP64-`AESGLOBAL`-Ablage festgelegt (gemlib retargeten)?
2. Danach **(A) gegen (B)**.
3. Erst danach sind die drei `#[ignore]` aufloesbar.
