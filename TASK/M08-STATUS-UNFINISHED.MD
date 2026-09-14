# M08 — STATUS: UNFINISHED (2026-09-14)

**Commit `29cfd02`.**

**UNFINISHED heisst hier: das Werkzeug ist fertig und belegt, die dritte Tabelle
haengt an einem Produktdefekt, den dieser Auftrag nicht anfassen durfte.**

Die Verweigerung ist gefallen. `mkrusttests` emittiert die achte Spalte, der
Rust-Harness haelt die vier Dumper-Puffer, und zwei der drei bisher unbewachten
Tabellen laufen ab sofort bei jedem `cargo run -p xtask -- test` mit. Die dritte
ist **gemessen, emittierbar und absichtlich nicht verdrahtet** — mit Grund,
Quelle und Zeilennummer.

## 1. Die Vorbedingung — zuerst, und sie war real

Der Nachtrag vom 13.09. hatte recht, und zwar nicht theoretisch. Der Emitter ist
opcode-blind; mit `aes-m06-sysvar.tsv` verdrahtet und **ohne** Fix:

```
run 1: 194 passed; 3 failed      run 5: 196 passed; 1 failed
run 2: 196 passed; 1 failed      run 6: 195 passed; 2 failed
run 3: 196 passed; 1 failed      run 7: 196 passed; 1 failed
run 4: 195 passed; 2 failed      run 8: 194 passed; 3 failed
```

**Acht von acht Laeufen rot**, jedes Mal an anderer Stelle
(`cargo test -p tos-api --test aes_reference -- -Zunstable-options --shuffle`).
Ein Beispielfehler:

```
tosdump/cases/aes-m06-sysvar.tsv case 90 (opcode 48) -- get LK3DIND: ... defaults 0, 1
  2 field(s) differ from the EmuTOS 1.4 reference:
    int_out[1]  NewTOS = 0x0001 (1)   reference = 0x0000 (0)
    int_out[2]  NewTOS = 0x0000 (0)   reference = 0x0001 (1)
```

Das ist wortwoertlich, was Fall **107** (`set LK3DIND to 1,0`) hinterlaesst. Die
Tabelle sagt es selbst im Kopf: *„section A — which measures the DEFAULTS — must
come before any set, and this table has to be run on a freshly started AES."*
Der Dumper faehrt eine Tabelle **einmal, in Reihenfolge, auf einer frisch
gestarteten AES**; cargo faehrt einen `#[test]` pro Fall in beliebiger Folge in
**einem** Prozess.

### Was gewaehlt wurde, und warum diese Form

Der Harness hat fuer genau dieses Problem schon eine Antwort, und sie steht in
seinem eigenen Kopfkommentar:

> *A declared reset is issued **after** the case as well as before it. The dumper
> needs no such thing … but here round 1 and round 2 share one test binary in an
> arbitrary order … So the trailing reset is what keeps cargo's ordering
> irrelevant.*

Also dieselbe Form, nur fuer den Zustand, den `wind_new` nicht sweept:
`sweep_3d_look` (`tos-api/tests/aes_reference/harness.rs`) laeuft **vor und
nach** jedem Fall und stellt die sieben 3D-Variablen auf die Vorgaben von
`init_3d` zurueck (`references/emutos/aes/gemoblib.c`, `init_3d`).

Drei Eigenschaften, die die Wahl tragen:

* **Ueber die AES-eigene Set-Seite**, nicht ueber einen Griff in
  `tos-api/src/aes/object.rs`' Statics. `ob_sysvar(mode=1, …)` ist der einzige
  Schreiber, den EmuTOS hat, und der einzige, den NewTOS hat — der Sweep kann
  also nichts wiederherstellen, was die API nicht selbst herstellen koennte.
  **Kein Produktcode angefasst.**
* **Er reproduziert den Dumperzustand exakt, Fall fuer Fall.** Jeder
  Rueckleseefall der Tabelle traegt sein eigenes Prelude (`+48 … set it again,
  for the read-back`), also sieht jeder Fall mit „Vorgaben + eigene Preludes"
  genau den Zustand, den der Laufende an dieser Zeile erreicht hatte —
  einschliesslich Fall 116, dessen Pointe ist, dass der abgewiesene
  `BACKGRCOL 32767` nichts geaendert hat.
* **Die Farbvorgabe wird abgeleitet, nicht hingeschrieben.** EmuTOS waehlt
  `WHITE` bei `ws_ncolors <= LWHITE`, sonst `LWHITE`; der Harness rechnet das aus
  seinem eigenen `SCREEN_COLORS` (16) aus.

### Verworfen, mit Grund

| Weg | Warum nicht |
|---|---|
| **Eine Tabelle = ein `#[test]`, Faelle in Tabellenreihenfolge** | Die treueste Form (genau was der Dumper tut), kostet aber die Fallgranularitaet: ein roter Fall traegt keinen eigenen Namen mehr, und die Suite verliert 19 benannte Faelle fuer einen. |
| **Prefix-Replay: jeder Fall spielt erst alle vorangehenden Faelle seiner Tabelle** | Ebenfalls treu und ordnungsunabhaengig, aendert aber, **was 173 heute gruene Faelle messen** (r1/r2 laufen dann mit kumuliertem statt frischem Zustand). Blast radius weit ausserhalb M08. |
| **`--test-threads=1` ohne `--shuffle`** | Verlaesst sich auf eine **nicht zugesicherte** Reihenfolge von libtest. Genau die Falle, die der Nachtrag beschreibt, nur unsichtbar. |
| **Eine neue `Reset`-Variante, von der Tabelle deklariert** | Unmoeglich: `aes-m06-sysvar.tsv` ist CRC-gebunden und bereits auf beiden Maschinen gefahren; eine Zeile `!reset` mehr macht jeden vorhandenen Dump ungueltig. Und der Emitter darf die Variante nicht raten — er ist mit Absicht opcode-blind. |
| **`#[ignore]` auf Abschnitt A** | Das bewacht nichts, es versteckt. |
| **Ein `reset_3d()` im Produktcode** | Unnoetig — die Set-Seite reicht — und ausserhalb des Auftrags. |

### Der Preis, ausdruecklich

**Eine Falltabelle kann keinen 3D-Wert mehr messen, der von Fall zu Fall
ueberlebt.** Sie muss den Zustand, den sie misst, in ihren eigenen Preludes
aufbauen — so wie m06 es tut. Das ist der einzige Verlust; die Faelle laufen
weiterhin parallel, serialisiert von `AES_LOCK` wie vorher. Der Kosten-Satz
steht im Doc-Kommentar von `sweep_3d_look`, nicht nur hier.

### Nachweis nach dem Fix

Fuenf geschuettelte Laeufe, dazu fuenf einthreadige:

```
run 1..5  (shuffle):              204 passed; 0 failed   (je ~0,43 s)
run 6..10 (shuffle, 1 thread):    204 passed; 0 failed
```

## 2. Die eigentliche Arbeit — die vier Puffer

`tos-api/tests/aes_reference/harness.rs`:

* `BUF_N = 4`, `BUF_BYTES = 256`, `BUF_TEXT_LEN = 8` — die Zahlen aus
  `tosdump/core/tosdump.h`.
* `buf_fill` ist `td_aes_buf_fill` (`tosdump/core/record.c`): jedes Byte
  `'A' + (index * 7 + i) % 26`, NUL an Byte 8, dieselbe druckbare Folge dahinter
  als Kanarie. **Kein Wortmuster** — die Aufrufe lesen C-Strings durch den
  Zeiger und keine der beiden AES begrenzt sie.
* Die vier liegen als **Statics** (einmal geleakt), nicht in der `Pb`: ein
  Aufruf darf den Zeiger behalten (`wind_set(WF_NAME)` legt ihn in die
  Fenstertabelle, ein spaeteres `wind_get` liest ihn zurueck), also wuerde ein
  `Pb`-eigener Puffer beim Fallende baumeln.
* Sie werden **vor jedem Aufruf** neu gelegt, in `Pb::poison` — genau an den
  drei Stellen, an denen `runner_aes.c` `lay_buffers` ruft.
* `Pb::plant` setzt `aN=B` in `addr_in[N]` und `iN=B` als zeigerbreiten Wert
  ueber die `int_in`-WORDs (LP64: 8 Byte, unaligned geschrieben wie der
  `memcpy` des Dumpers). Ein `iN`-Plant auf einem **gemessenen** Fall wird
  abgewiesen, wie der Lader ihn abweist.
* `Pb::recorded_addrin` gibt das **Token** `0xB0FF0000 + n` zurueck, solange der
  Schlitz haelt, was gepflanzt wurde, sonst die unteren 32 Bit — `runner_aes.c`
  Schritt 6, `TD_AES_ADDR_TOKEN`.

`tosdump/host/mkrusttests.c`: die Verweigerung ist ersatzlos entfallen;
`emit_plants` schreibt die Spalte, die Importzeile nimmt `Kind`/`Plant` nur auf,
wenn die Tabelle wirklich pflanzt (eine unbenutzte Einfuhr ist eine Warnung),
und der Modulkopf erklaert die achte Spalte, wenn die Tabelle sie deklariert.
Die VDI- und Raster-Emitter sind unveraendert (nachgeprueft: neu erzeugt,
bytegleich).

**Belegt, nicht angenommen.** Zwei handgeschriebene Dauertests in
`tos-api/tests/aes_reference.rs`: einer prueft alle vier Puffer Byte fuer Byte
(die vier Ketten `ABCDEFGH` / `HIJKLMNO` / `OPQRSTUV` / `VWXYZABC`, die NUL und
**jedes** Kanariebyte bis 255), einer das Token — gepflanzt, ueberschrieben,
und nach einem frischen `poison` wieder Gift. Der erste ist noetig, weil
`appl_find` fuer **jeden** Namen `-1` antwortet, der kein AES-Prozess ist: die
m05-Faelle waeren auch mit falschen Bytes gruen geworden.

Dass der Mechanismus wirklich greift, steht ausserdem in der Aufnahme: die
m05-Datensaetze tragen `addr_in[0] = 0xb0ff0000` bzw. `0xb0ff0001`, und der
Harness vergleicht dagegen.

## 3. Was jetzt bewacht ist

| Tabelle | Faelle | Zustand |
|---|---|---|
| `tosdump/cases/aes-m05-addrin.tsv` | 5 | gruen, verdrahtet |
| `tosdump/cases/aes-m06-sysvar.tsv` | 19 | gruen, verdrahtet |
| Puffer-/Token-Selbstpruefung | 2 | gruen, verdrahtet |
| `tosdump/cases/aes-m07-parameter.tsv` | 20 | **nicht verdrahtet**, siehe unten |

`cargo run -p xtask -- test`: **504 -> 530 gruen / 0 rot / 4 ignoriert.**

**m05 Fall 102 ist der Lohn des Ganzen.** Die Aufnahme vom 12.09. hatte dort
`int_out[0] = 1` auf NewTOS gegen `0` auf dem TT; A42 hat das am 13.09.
repariert (`wind_get(WF_NAME)` verweigert wie `wm_get`s `default:`-Arm). Der
Fall ist heute gruen — und ab jetzt **dauerhaft bewacht**, statt einmal
gemessen.

## 4. Zwei Befunde aus `aes-m07-parameter.tsv` — gemeldet, nicht repariert

Die Tabelle wurde erzeugt, verdrahtet und gefahren. Zehn von zwanzig Faellen
rot. Deshalb ist sie **wieder ausgebaut**: eine zur Haelfte rote Suite ist kein
Waechter, und keiner der beiden Gruende liegt im Auftragsumfang.

### B-1 — Der Mauszeiger parkt einen Pixel daneben (10 Faelle)

```
int_out[1]  NewTOS = 0x0140 (320)   reference = 0x013F (319)
int_out[2]  NewTOS = 0x00F0 (240)   reference = 0x00EF (239)
```

* **NewTOS**: `GCURX = backend.width() / 2`, `GCURY = backend.height() / 2`
  (`tos-api/src/vdi/mouse.rs`, `ensure_init`) → 320 / 240 bei 640×480.
* **EmuTOS**: `GCURX = xres / 2; GCURY = yres / 2;`
  (`references/emutos/vdi/vdi_mouse.c:582-583`) mit
  `#define xres DEV_TAB[0]` (`vdi/vdi_defs.h:94`) und
  `DEV_TAB[0] = V_REZ_HZ - 1` (`vdi/vdi_control.c:203-204`) → 639/2 = **319**,
  479/2 = **239**.

Das ist **keine** Maschineneigenschaft. Beide Zahlen sind „die Parkposition,
berechnet aus der Bildschirmgroesse"; die beiden Rechnungen unterscheiden sich.
Der VDI-Harness hat fuer echte Maschineneigenschaften `MACHINE_EXCLUDED` **mit
Ersatzpruefung gegen EmuTOS' Formel** — genau diese Ersatzpruefung wuerde NewTOS
hier nicht bestehen. Also ein Defekt, kein Ausnahmefall.

Reparatur ist Produktcode in `tos-api/src/vdi/mouse.rs` und veraendert die
Parkposition des Desktops auf echter Hardware — eigenes Ticket, eigene Abnahme.
**Am Tag der Reparatur landet die Tabelle.**

### B-2 — Eine stehengebliebene Nachricht laesst `MU_MESAG` feuern (Fall 69)

```
case 69 (opcode 25) -- MU_MESAG|MU_TIMER with a planted MME_PBUFF
    int_out[0]  NewTOS = 0x0030 (48)   reference = 0x0020 (32)
```

`0x30` = `MU_MESAG | MU_TIMER`. Der Fall meldet eine Nachricht, wo der TT nur
den Timer meldet. **Allein gefahren ist der Fall an dieser Stelle gruen**
(`cargo test --test aes_reference cases_m07_parameter`, nur die zwei
Mauswoerter weichen ab) — die Nachricht stammt also aus einem Fall einer
**anderen** Tabelle: r2/m05 erzeugen und oeffnen Fenster, und was dabei in die
Warteschlange der Anwendung faellt, liest bis heute niemand.

Dieselbe Klasse wie die 3D-Flatterstelle, aber **nicht mit demselben Mittel
loesbar**: der Harness kann die Schlange ueber die AES-API nicht leeren, weil es
keinen nichtblockierenden Abzug gibt (`evnt_mesag` blockiert auf leerer
Schlange — `tosdump/core/cases.h` sagt das gleiche fuer den Dumper). Ein
moeglicher Weg, ungetestet und darum hier nur benannt: `evnt_multi` mit
`MU_MESAG|MU_TIMER` und Timer 0 in einer Schleife, bis nur noch `MU_TIMER`
feuert — das ist ein reiner AES-Abzug und braucht einen gepflanzten
`MME_PBUFF`, den es seit heute gibt. **Nicht gebaut**, weil heute nichts ihn
faehrt und ungefahrener Harnesscode genauso verrottet wie ein unverdrahteter
Selftest.

## 5. Nebenbefund ausserhalb des Umfangs

NewTOS hat kein `init_3d`: `IND_BUTCOL`/`ACT_BUTCOL`/`BACKGR_COL` sind
Compile-Zeit-Konstanten `GRAY_LT` (= EmuTOS' `LWHITE`, 8) und nehmen EmuTOS'
Zweig `ws_ncolors <= LWHITE -> WHITE` nie
(`references/emutos/aes/gemoblib.c`, `init_3d`). Auf einer Workstation mit
**acht oder weniger** Stiften waeren die drei Vorgaben deshalb 8 statt 0. Nicht
angefasst; auf dem 16-Stift-Profil dieses Harness und auf dem gemessenen Profil
beider Maschinen unsichtbar.

## 6. Was gebaut und gemessen wurde

* `cargo run -p xtask -- test`: **530 gruen / 0 rot / 4 ignoriert** (Ausgangslage
  504 / 0 / 4), gemessen um 14:0x.
  **Achtung:** eine spaeter am selben Tag eingetroffene, **fremde und nicht
  committete** Aenderung an `tools/mkrsc.py` + `tos-api/src/aes/resource.rs`
  (NRSC v7, relokierbarer Baumindex) macht `xtask`s
  `mkrsc_convert_lp64_reserves_the_declared_tedinfo_lengths` rot
  (`range start index 65536 out of range for slice of length 556`,
  `tools/xtask/src/rsc_align_tests.rs:452`). Sie gehoert **nicht** zu diesem
  Aenderungssatz und wurde nicht angefasst. Ohne dieses Paket:
  `cargo test -p tos-api -p tos-types` = **520 gruen / 0 rot / 4 ignoriert**.
* `tosdump/host`: `make test` vollstaendig gruen (`-Werror`).
* Beide Boards frisch gebaut, `--release`, mit erzwungener Neuuebersetzung
  (`touch` auf die drei Crate-Wurzeln vor jedem Lauf) und unterschiedlichen
  ELF-Pruefsummen als Beleg, dass wirklich zweimal gebaut wurde:
  `board-rpi4` `d35d8003…`, `board-qemu-virt` `cb916afe…`.
* Warnungen: `tos-api` 64 rustc / 82 clippy, `kernel` 94 rustc / 125 clippy.
  Die 82 gegen die Grundlinie 81 stammen **nicht** aus diesem Aenderungssatz —
  er fasst keine Zeile unter `tos-api/src/` an; die Verteilung liegt in
  `aes/object.rs` (17), `aes/shell.rs` (15), `aes/window.rs` (11),
  `aes/menu.rs` (10), `vdi/mouse.rs` (8), `aes/form.rs` (8), `aes/resource.rs`
  (6, fremd uncommitted), `aes/fsel.rs` (6), `bdos/fs/find.rs` (1).
  Die beruehrten Testziele sind clippy-sauber (das neue
  `needless_range_loop` sofort behoben).

## 7. Offen

* **`aes-m07-parameter.tsv` verdrahten** — haengt an B-1 (Produktreparatur in
  `tos-api/src/vdi/mouse.rs`) und danach an B-2 (Schlangenabzug im Harness).
* **Abnahme durch den Tester.** Hostsuite und Buildbeleg sind meine; das
  Verhaltensurteil nicht.
