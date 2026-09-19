# A60 — Quittung

**Status: BEFUND BELEGT (Messung in QEMU raspi4b) — Fix OFFEN (Overlay-Vorschlag
unten, nichts geaendert)** · 2026-09-19 · Auditor.

Kein Produktcode angefasst: Trace nur in der Scratch-Kopie
(`measurement-2026-09-19-a60/method/scratch-*.diff`), `kernel/src/host_env.rs`
sha256 `287ff741…` vor und nach der Messung, `git status --short` zeigt nur
`measurement-2026-09-19-a60/` und diese Datei als neu.

## 1. Die drei Dumps (TeraDesk, AES-pid 1, `global` = VA `0x3258c`)

Trace: bei jedem AES-Trap aus pid 1 die 32 Byte ab dem `global`-Zeiger des
AESPB, plus die Rueckgaben von `rsrc_load`/`rsrc_gaddr`, plus TeraDesks eigene
Statics `svicntree`/`svicnrshdr`/`icons` (Link-VAs `0x30f60`/`0x30f68`/`0x30ed0`
aus `llvm-nm dist/DESKTOP.NAP`) und `Malloc`-Rueckgaben. Rohdaten
`serial/run1-serial.log:42-181`, Dekodierung `decode-run1.txt`.

Zuordnung Byte → Feld: **TOS-Ablage** nach `docs/api/aes/application.md`
§ `AESGLOBAL` (30 Byte, Zeiger 4 Byte, 2-Byte-ausgerichtet) und **TeraDesks
`AES_GLOBAL`** (`library/xdialog/library.h:65-77`, mit dem SDK-clang fuer
`aarch64-unknown-none-elf` ausgemessen, `method/offs.c`: `ap_private` @8,
`ap_ptree` @16, `ap_rscmem` @24, `ap_rsclen` @32, `ap_planes` @34, `ap_3resv`
@40, `sizeof` 56 — die drei letzten liegen jenseits der 32 Byte von
`short aes_global[16]`, gemlib `mt_gem.h:2503` `AES_GLOBMAX 16`).

| Punkt | Bytes 0..31 (je 2 Byte) | TOS-Ablage | `library.h`-Struktur |
|---|---|---|---|
| nach `appl_init` (`:44`) | `4003 0100 0100 7b11 0000 0000 0000 0000 0000 0000 1000 185b 1200 0000 0000 0000` | `ap_version` 0x0340, `ap_count` 1, `ap_id` 1, `ap_private` 0x117b, `ap_ptree`@10 0, `ap_rscmem`@14 0, `ap_rsclen`@18 0, `ap_planes`@20 16, `ap_3resv`@22 0x00125b18 | `ap_ptree`@16 = `0x5b18001000000000` (= `ap_planes`‖`ap_3resv`-low16), `ap_rscmem`@24 = `0x12` (= `ap_3resv`-high16) |
| nach `rsrc_load("desktop.rsc")` = 1 (`:47`) | `4003 0100 0100 7b11 0000 4400 4000 0000 4000 3699 1000 185b 1200 0000 0000 0000` | `ap_ptree`@10 **0x00400044** (Blob 0x400000 + 68-Byte-Kopf), `ap_rscmem`@14 **0x00400000**, `ap_rsclen`@18 **39222**; Rest wie oben | `ap_ptree`@16 = `0x5b18001099360040`, `ap_rscmem`@24 = `0x12` |
| erster Trap nach `load_icons` (`:180`, op 104) | `4003 0100 0100 7b11 0000 2870 4200 0070 4000 3699 1000 185b 1200 0000 0000 0000` | `ap_ptree`@10 **0x00427028** (icons.rsc-Baumfeld — **nicht** auf desktop.rsc zurueckgestellt), `ap_rscmem`@14 **0x00407000** (**Hybrid**: low16 von 0x427000, high16 von 0x400000), `ap_rsclen`/`ap_planes`/`ap_3resv` zurueckgestellt | `ap_ptree`@16 / `ap_rscmem`@24 = exakt die Werte nach `rsrc_load` (die 16-Byte-Rueckstellung `stp x22, x21, [x20, #0x10]`, `load_icons+0x140`) |

Dazu gemessen: `Malloc(13284) -> 0x427000` (`:179`, der `xrsrc_load`-Puffer);
`icons.rsc` traegt `rsh_trindex = 0x0028` (native RSHDR, Dateibyte 0x12), also
Baumfeld `0x427028`; TeraDesk-Statics nach `load_icons` (`:181`):
**`svicntree = 0x5b18000000000042`, `svicnrshdr = 0x12`**, `icons = 0x427050`.

Der Zwischenstand (nach `xrsrc_fix`, vor der Rueckstellung) ist ohne AES-Trap
dazwischen nicht direkt dumpbar; er ist aus den zwei 8-Byte-Stores hergeleitet
und **von beiden Messwerten bestaetigt** (`decode-intermediate.txt`): Store
`0x427028` @10 (8 B), dann `0x427000` @14 (8 B) ergibt
`… 28 70 42 00 | 00 70 42 00 | 00 00 00 00 | 18 5b 12 00 …` — die zweite
Schreibung ueberschreibt Byte 14..17 (obere Haelfte des ersten Zeigers) und
**nullt Byte 18..21** (`ap_rsclen`, `ap_planes`) transient. Byte 16..23 als
u64 = `0x5b18000000000042` = gemessenes `svicntree`; Byte 24..31 = `0x12` =
gemessenes `svicnrshdr`. Die Rueckstellung von Byte 16..31 liefert dann
Byte fuer Byte den gemessenen Dump nach `load_icons`.

## 2. Der Widerspruch als Zahl — CONFIRMED

| | Schreiber `rsc_load.c:417-422` (`stur x8,[x9,#0xa]` / `stur x19,[x9,#0xe]`, `xrsrc_load+0x4c8`) | Leser `icon.c:2751-2752` via `library.h:81,83` (`ldp x9, x11, [x20, #0x10]`, `load_icons+0xe8`) |
|---|---|---|
| `ap_ptree` | schreibt **0x427028** (8 B @10) | liest **0x5b18000000000042** (8 B @16) |
| `ap_rscmem` | schreibt **0x427000** (8 B @14) | liest **0x12** (8 B @24) |

Ein Lauf, in dem TeraDesk einen Wert schreibt und einen anderen zurueckliest —
die Abnahme des Tickets. Beide Rueckleseergebnisse sind wilde Zeiger, aus
`ap_3resv` (Kernel-Static-Adresse, `appl.rs:157`) und Nullbytes gebaut.

**Heute sichtbar: nichts — bestaetigt.** Screendump `run1-after-boot.png`:
Menueleiste, vier Laufwerks-Icons, Papierkorb, Mono-Icons aus `icons.rsc`.
Gruende, jeder einzeln geprueft:

* `xrsrc_gaddr`/`xrsrc_free` lesen 8 Byte @14 (`ldur x8,[x8,#0xe]`,
  `xrsrc_gaddr+0xc`; `xrsrc_free+0x8`) — dieselbe falsche Annahme wie der
  Schreiber, also konsistent: `icons` wird richtig aufgeloest.
* Der transiente Nullstand von `ap_rsclen`/`ap_planes` liegt zwischen zwei
  AES-Traps (`graf_handle` in `rsc_obfix` kommt **vor** den Stores,
  `rsc_load.c:224`); kein Leser dazwischen; die Rueckstellung bringt beide
  zurueck (`36 99 10 00` im Dump `:180`).
* Die Kern-`rsrc_*` lesen das `AESGLOBAL` heute nicht (A39 Stufe 1,
  `resource.rs:553-557`), also stoert `0x427028`/`0x407000` nichts.
* `free_icons` (`icon.c:2769-2785`, Ende) schreibt `svicntree`/`svicnrshdr`
  nach Byte 16..31 zurueck; `xrsrc_free` liest dann 8 Byte @14 = Byte 14..15
  (`00 70`, noch vom Store) ‖ Byte 16..21 (`42 00 00 00 00 00` aus
  `svicntree`) = **0x427000** — `Mfree` trifft den richtigen Puffer **durch
  Zufall** der Ueberlappung (hergeleitet aus `free_icons+0x30`, nicht
  gemessen: braucht die Quit-Geste).
* `aes_supports_coloricons` (`icon.c:2692`, `ldr x9,[x19,#0x18]`) laeuft nur
  nach erfolgreichem Kern-`rsrc_load("cicons.rsc")`; der liefert 0 (Datei
  fehlt; **run2** mit der unkonvertierten `cicons.rsc` auf der Karte: Trace
  und Schirm bytegleich, der Kern lehnt das Format ab).

## 3. Was A39 Stufe 2 daraus laese

Selektor = u32 @14 des Aufrufer-`AESGLOBAL`, verglichen mit `blob_target` der
eigenen Eintraege (`resource.rs:543-549`; Analyse § 4.2 R3). Nach `load_icons`
steht dort **0x00407000**: weder die Kern-Basis von `desktop.rsc` (`0x400000`)
noch der private `icons.rsc`-Block (`0x427000`, nicht in der Kernliste) →
„nothing loaded" → jedes spaetere `rsrc_gaddr(R_STRING, …)` = 0 (Infozeile,
Dialoge, Fehlertexte; die 36 Aufrufe aus H05-run2 sind genau diese), und
`rsrc_free()` am Ende (`main.c:1992`) = 0. `ap_ptree`@10 = `0x427028` naehme
R4 als Baumfeld — TeraDesks privates Feld, nicht der Kern-Blob. Randnotiz:
`0x407000` liegt **innerhalb** von `[0x400000, 0x400000+39222)`; ein
Bereichs- statt Basisvergleich traefe desktop.rsc zufaellig — R3 vergleicht
die Basis, das ist richtig so.

Im Leerlauf-Boot folgt nach `load_icons` kein `rsrc_gaddr` (`:182-185` nur
`shel_find`); die Wirkung zeigt sich beim ersten Fenster/Dialog.

## 4. Vorschlag des Fixes — Overlay, keine Aenderung vorgenommen

Es gibt keinen 32-Bit-Zeigertyp in C auf LP64; ein lvalue, das 4 Byte
speichert und `OBJECT **` ergibt, ist nicht ausdrueckbar. Deshalb haben
gemlibs Overlay-Makros die Setter `_AESrscfile_set(p)`/`_AESrscmem_set(p)`
(`sdk/gemlib/overlay/gem.h:103-124`, `mt_gem.h.in:2560-2564`) und machen
die Zuweisung `_AESrscfile = x` absichtlich zum Compilefehler. TeraDesk umgeht
das an **zwei** Stellen: `library.h` definiert die Makros um, `rsc_load.c`
adressiert `&aes_global[5]/[7]` direkt. Drei Overlay-Dateien unter
`sdk/overlays/teradesk/` (Schattenbaum-Mechanik `build-teradesk.sh:104-125`,
Muster `window.h`/`environm.c`; LP64-Korrekturen sind erlaubter Inhalt,
`docs/sdk/04-porting-anatomy.md` § „What kinds of change live in an overlay"):

1. **`library/xdialog/library.h`** (257 Zeilen, Kopie): Zeilen 55–83
   streichen — die `aes_private`/`AES_GLOBAL`-Typedefs und die vier Zeilen
   `#undef/#define _AESrscfile`/`_AESrscmem`. Damit gelten gemlibs
   Overlay-Leser (4 Byte @10/@14, `gem.h:104,117`). `AES_GLOBAL`/`aes_private`
   hat keinen weiteren Nutzer im Baum (`/usr/bin/grep -rn` ueber
   `sdk/apps/teradesk`). Wer die Struktur als Dokumentation behalten will,
   legt sie mit `__attribute__((packed))` und `unsigned int`-Feldern in der
   TOS-Ablage an — sie darf dann aber nicht als Zugriffsweg dienen.
2. **`icon.c`** (3229 Zeilen, Kopie): die acht Zuweisungen
   `_AESrscfile = …;`/`_AESrscmem = …;` in `:2742-2743`, `:2762-2763`,
   `:2776-2777`, `:2781-2782` → `_AESrscfile_set(…);`/`_AESrscmem_set(…);`.
   Die Leser `:2692`, `:2707-2708`, `:2751-2752` uebersetzen unveraendert
   (die Overlay-Makros liefern `OBJECT **`/`void *`).
3. **`rsc_load.c`** (664 Zeilen, Kopie): die drei `#ifdef __GNUC__`-Arme
   (`:414-427` Stores, `:482-490` `xrsrc_free`, `:501-509` `xrsrc_gaddr`) →
   `_AESrscfile_set(_rs_trindex); _AESrscmem_set(buf);`, `Mfree(_AESrscmem);`,
   `useRsc = _AESrscmem;` (das ist wortgleich der jeweilige `#else`-Arm, nur
   mit Setter). `rsc_load.h:84-88` beschreibt genau diesen Vertrag:
   „aes_global[5/6] … aes_global[7/8]" — TOS-Worte.

Ergebnis nach dem Fix: `load_icons` sichert u32@10/@14 (`0x400044`/
`0x400000`), `xrsrc_fix` schreibt `0x427028`/`0x427000` als u32 (keine
Nullung von Byte 18..21 mehr), `svicntree`/`svicnrshdr` = `0x427028`/
`0x427000`, die Rueckstellung setzt @10/@14 auf desktop.rsc — EmuTOS'
`rs_sglobe`-Semantik (`gemrslib.c:673-679`), Vorbedingung fuer A39 Stufe 2
und A41 (`aes_supports_coloricons` vergleicht dann gegen das echte
`ap_rscmem`, und die Rueckstellung nach einem Kern-`rsrc_load(cicons.rsc)`
trifft die Felder, die der Kern schreibt).

**Nachweis-Test danach** (dieselbe Methode, `method/a60_patch.py` + `run.sh`):

* `sdk/apps/build-teradesk.sh`: „overlays applied: 6", `FAILED=0`;
  `llvm-objdump -d` von `load_icons`: kein `ldp/stp …, [x, #0x10]` mehr,
  stattdessen 4-Byte-`ldur/stur w…, [x, #0xa]`/`[#0xe]`; `xrsrc_gaddr`/
  `xrsrc_free`: `ldur w…, [x8, #0xe]`.
* Trace: der erste Trap nach `load_icons` zeigt den Dump **bytegleich** zum
  Stand nach `rsrc_load(desktop.rsc)` (`… 4400 4000 0000 4000 3699 1000 …`);
  Statics `svicntree = Malloc-Ergebnis + 0x28`, `svicnrshdr = Malloc-Ergebnis`.
* Screendump bytegleich zu `run1-after-boot.png` (kein sichtbarer Unterschied
  erwartet — die Mono-Icons kommen so oder so aus dem privaten Lader).
* Danach erst A39 Stufe 2 bauen; dessen Abnahme: `rsrc_gaddr(R_STRING, …)`
  nach `load_icons` = 1 (Fenster oeffnen: Infozeile), `rsrc_free()` am Ende
  = 1.

## Was ich gefahren habe / nicht verifizieren konnte

* Gefahren: Boot mit `#Z`-Autostart, Dump an drei Punkten, Statics-Rueckleser,
  `Malloc`-Basis, Zweitlauf mit `cicons.rsc`; Offsets von `AES_GLOBAL` per
  Compiler; alle drei Zugriffsformen im Disassemblat von `DESKTOP.NAP`.
* Nicht gefahren (Geste noetig): der `free_icons`-Pfad beim Beenden (nur
  hergeleitet) und die Stufe-2-Wirkung selbst (Stufe 2 ist nicht gebaut).

## Nebenbefunde (unverfolgt)

* `colour_icons = TRUE` kommt aus `xdialog.c:2446-2451` (`get_tosversion() >=
  0x400 && aes_version >= 0x330`, ohne `appl_getinfo`), also laeuft der
  `rsrc_load("cicons.rsc")`-Zweig auf NewTOS immer und scheitert immer.
* Die 8-Byte-Schreibung in `xrsrc_fix` nullt `ap_rsclen`/`ap_planes`
  transient — heute unbeobachtbar, mit dem Overlay weg.

Artefakte: `measurement-2026-09-19-a60/` (RUNINFO.TXT, serial/, screendumps/,
method/, build/, decode-*.txt, manifest.sha256).
