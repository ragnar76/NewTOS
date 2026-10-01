# T01 — Quittung

**Status: IMPLEMENTIERT · quellen- und maschinenbelegt · Sichtpruefung der
Laufwerksinfo auf echter Pi offen** · 2026-09-17 · `_DISKINFO` ist im Overlay
auf feste 32-Bit-Felder mit `packed` umgestellt und durch `_Static_assert` je
Feldoffset **und** fuer `sizeof` an den Kern gebunden. Der Assert war vorher
rot, gefuehrt in TeraDesks eigener Uebersetzungseinheit.

Artefakte: [measurements/measurement-2026-09-17-diskinfo/](../measurements/measurement-2026-09-17-diskinfo/RUNINFO.TXT).

## Was geaendert wurde

Eine Datei: `sdk/libc/overlay/include/mint/ostruct.h`.

```c
typedef struct {
    __UINT32_TYPE__ b_free;
    __UINT32_TYPE__ b_total;
    __UINT32_TYPE__ b_secsiz;
    __UINT32_TYPE__ b_clsiz;
} __attribute__((packed)) _DISKINFO;

_Static_assert(__builtin_offsetof(_DISKINFO, b_free)   ==  0, "b_free@0");
_Static_assert(__builtin_offsetof(_DISKINFO, b_total)  ==  4, "b_total@4");
_Static_assert(__builtin_offsetof(_DISKINFO, b_secsiz) ==  8, "b_secsiz@8");
_Static_assert(__builtin_offsetof(_DISKINFO, b_clsiz)  == 12, "b_clsiz@12");
_Static_assert(sizeof(_DISKINFO) == 16, "sizeof(_DISKINFO) == 16");
```

Sollstand: `docs/api/gemdos/directory/36-dfree.md` — „einen caller-seitigen
16-Byte-Puffer (vier LONGs)". Der Kern schreibt genau das
(`tos-api/src/bdos/fs/drive.rs` `dfree`, vier 4-Byte-Speicherungen bei
`p.add(i * 4)`) und wurde **nicht** angefasst: `_DISKINFO` enthaelt keinen
Zeiger, laesst sich also — anders als `MFDB` — auf die Drahtfassung pinnen,
genau wie `_DTA` am 2026-07-25. Kein Rust geaendert.

## Eine Berichtigung zum Auftrag

Der Auftrag nimmt an, ein `sizeof`-Assert an `_DTA` haette `_DISKINFO` 2026-07
mitgezogen. Das stimmt nicht: `_DTA` **hat** diesen Assert seit 2026-07-25
(`_Static_assert(sizeof(_DTA) == 44, ...)`, in `0fe9bad` nachlesbar), und ein
Assert kann nur fuer die Struktur ausloesen, die er nennt. Kein Assert an
`_DTA` haette `_DISKINFO` je gefunden.

Der heute gebaute Schutz schuetzt ebenso nur `_DISKINFO`. Was 2026-07 fehlte,
war nicht der Assert, sondern der **Blick auf die Geschwister** -- genau das
leistet die beiliegende Erhebung aller 30 `mint/`-typedefs. Ein Assert ist
oertlich; erst die Erhebung macht ihn wiederholbar. Die Nachricht des Commits
`5a5820b` gibt die Auftragsannahme noch unberichtigt wieder.

## Test vor Fix

Der Assert zuerst ROT, in `showinfo.c`, mit dem Flaggensatz von
`sdk/apps/build-teradesk.sh` und aus dem Schattenbaum:

```
In file included from .../build/tree/showinfo.c:24:
In file included from .../build/tree/library/xdialog/library.h:85:
In file included from .../mintlib/include/mint/mintbind.h:10:
In file included from .../mintlib/include/mint/osbind.h:116:
.../mintlib/include/mint/ostruct.h:34:16: error: static assertion failed ...
   34 | _Static_assert(sizeof(_DISKINFO) == 16, "sizeof(_DISKINFO) == 16");
      |                ~~~~~~~~~~~~~~~~~~^~~~~
      note: expression evaluates to '32 == 16'
```

Dazu drei rote Offset-Asserts (`b_total` 8==4, `b_secsiz` 16==8, `b_clsiz`
24==12). Die Frage „welches Kopfstueck gewinnt" ist mitbeantwortet: `clang -H`
weist genau `sdk/libc/mintlib/include/mint/ostruct.h` aus — die Stelle, an die
`build-libc.sh` das Overlay kopiert. Volles Protokoll:
`measurements/measurement-2026-09-17-diskinfo/01-red-static-assert-showinfo.log`.

## Nachher

* **TeraDesk** neu gebaut: libc 613 Dateien (11 erwartete Ausfaelle, 0 neue) ->
  gemlib 578/578 -> TeraDesk **48/48**, LINK OK, **0 undefinierte Symbole**
  (`02-build-teradesk.log`).
* **Maschinenbeleg im Objekt** (`07-showinfo-disasm-before-after.txt`):
  vorher `ldp x8, x9, [sp,#0xd0]` / `ldp x20, x24, [sp,#0xc0]` / `mul x23,…`
  — 64-Bit-Felder, von denen zwei ausserhalb der 16 vom Kern beschriebenen
  Bytes lagen; nachher `ldp w8, w9, [sp,#0xc8]` / `ldp w20, w24, [sp,#0xc0]` /
  `mul w23, w9, w8`, also genau die vier 32-Bit-Worte auf 0/4/8/12.
* **Beide Uebersetzer** gegen denselben Kopfstuecktext nachgemessen: `_DISKINFO`
  jetzt 16 auf m68k **und** 16 auf AArch64 (`04-struct-sizes-both-toolchains.txt`).
* `cargo run -p xtask -- test`: **557 passed / 0 failed / 1 ignored**.
* Beide Boards nach `cargo clean` frisch, `--release`: tos-api **64** rustc /
  **81** clippy, kernel **94** / **125** — Grundlinie unveraendert, 0 Delta
  (kein Rust angefasst).
* QEMU raspi4b headless 640x480, TeraDesk per `#Z`-Autorun: steht, keine
  Ausnahme, und gegen die zuletzt abgelegte Aufnahme
  (`measurements/measurement-2026-09-17-a65/after/teradesk.png`) **0 von 307200**
  Bildpunkten verschieden.

## Was offen bleibt — und warum es offen bleiben MUSS

Der Fehler wird erst im **Informationsdialog** eines Laufwerks sichtbar
(Laufwerkssymbol waehlen -> File -> Show info). Der Dialog braucht eine
Eingabe; der headless-QEMU-Lauf hat keinen Eingabeweg. Der Screendump belegt
deshalb nur die Abwesenheit eines Rueckschritts, nicht die Reparatur.

Die Sichtpruefung ist als **Vorhersage** formuliert und an der Quelle geprueft
(`showinfo.c:984-1032`, mit `-D_TOS_ONLY` ist `_MINT_ = 0`, der KB/MB-Zweig
faellt weg): FLCLSIZ = `b_secsiz*b_clsiz`, FLSPACE = `b_total*clsize`,
FLFREE = `b_free*clsize`, FLBYTES = Differenz. Auf der QEMU-Karte liefert der
Kern (APITEST T12, eigener 4x32-Bit-Puffer) `free_cl=31842 tot_cl=32672
bps=512 spc=4`, der Dialog muesste dort also 2048 / 66912256 / 65212416 /
1699840 zeigen. Auf der Pi gelten dieselben Invarianten mit groesseren Zahlen
(Einzelheiten in RUNINFO.TXT).

## Die Nachbarschaft

Alle 30 typedef-Strukturen der `mint/`-Kopfstuecke selbst mit beiden
Uebersetzern nachgemessen; die Zahlen des Tickets stimmen, nach dem Fix
weichen **20 von 30** ab. Fuer diese 20 ist erhoben, ob sie heute von einer
gebauten Anwendung gelesen werden — Liste mit Belegstellen in
`measurements/measurement-2026-09-17-diskinfo/08-struct-reach-survey.md`:

* **gelesen: 3** — `OSHEADER` (TeraDesk `gettos.c`, APITEST T04; sicher nur,
  weil `os_version` ein `unsigned short` auf 2 ist. Berichtigung zum Ticket:
  verschoben ist alles ab **`reseth`@0x04**, nicht erst ab `os_beg`@0x08),
  `MFDB` (TeraDesk + QED ueber cflib; korrekt, weil der Kern nach ABI
  verzweigt), `_KEYTAB` (TeraDesk `xdevent.c`, QED ueber cflib `nkccinit.o`;
  korrekt, weil `keytbl.rs` in der Breite des Aufrufers schreibt).
* **nicht gelesen: 17** — je mit Grund (Funktion nicht implementiert, kein
  Aufrufer, oder Bibliotheksobjekt wird von keinem Link gezogen).
* Ein Befund fuer spaeter: `_MD`/`_MPB` waeren mit blossem Verbreitern
  **nicht** repariert — die Fassung des Kerns (ADR-023) ist 24 Byte
  (Zeiger 8 / Long 4 / Long 4 / Zeiger 8), also weder m68k-16 noch LP64-32.
  APITEST umgeht das heute mit einem eigenen `MDX`.

## Grenzen eingehalten

`sdk/apps/teradesk/` unberuehrt (die Korrektur sitzt im Overlay, der
Schattenbaum bleibt symbolisch). Kein Hardwarezugriff; die Karte des Anwenders
unter `/Volumes` wurde nicht eingebunden (`method/prep-card.sh` prueft den
Einhaengepunkt gegen die selbst angehaengte Scheibe). Zwischendateien lagen
ausserhalb von `/tmp`.
