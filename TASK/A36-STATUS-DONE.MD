# A36 — Quittung

**Status: IMPLEMENTIERT · hosttestbelegt · Abnahme durch den Tester offen**
· 2026-09-13 · Die Relokation der Substruktur-Reihen ist auf EmuTOS' arrayweisen
Lauf umgestellt, vier vorher rote Hosttests sind gruen, beide Boards bauen.
**Nicht committet** — der Arbeitsbaum traegt in denselben Dateien unversionierte
Fremdarbeit (siehe Abschnitt „Warum nichts committet ist").

## Quelle

`references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`, Zielprofil
Falcon030/TOS4. Befund **B4** des Audits vom 2026-09-11.

## Der Befund, selbst nachgeprueft

`rs_readit` relokiert die Substrukturen **arrayweise** ueber die Kopfzaehler
(`aes/gemrslib.c:798-805`):

```c
fix_tedinfo_std();                        /* rsh_nted  */
ibcnt = rs_hdr->rsh_nib;
fix_nptrs(ibcnt, R_IBPMASK);
fix_nptrs(ibcnt, R_IBPDATA);
fix_nptrs(ibcnt, R_IBPTEXT);
fix_nptrs(rs_hdr->rsh_nbb, R_BIPDATA);
fix_nptrs(rs_hdr->rsh_nstring, R_FRSTR);
fix_nptrs(rs_hdr->rsh_nimages, R_FRIMG);
```

`fix_nptrs` (`:640-646`) ist `for (i=0; i<cnt; i++) fix_long(get_addr(type, i))`,
und `get_addr` (`:112-176`) indiziert die Reihenbasis aus dem Kopf — **kein**
Objekt ist beteiligt. Die Objekte laufen getrennt und spaeter, in `rs_fixit` →
`fix_objects` (`:606-637`, `rs_load` `:846-847`), das nur `ob_spec` anfasst.

NewTOS lief objektgetrieben: ein `G_TEXT`/`G_ICON`/`G_IMAGE`-Objekt fuehrte zu
seiner Substruktur, `substruct_seen` entdoppelte ueber die bereits bearbeiteten
Objekte. Ein Satz, auf den kein OBJECT zeigt, behielt rohe Dateioffsets.

Selbst nachgemessen an der ausgelieferten `tos-api/src/aes/gem_fsel.nrsc`
(NRSC v5, `n_ted=13`, `n_ib=0`, `n_bb=3`): sie enthaelt **kein** `G_IMAGE`-Objekt
(Typen 20/21/22/25/26/27/28/30), die drei BITBLKs haengen allein an der
Freibildtabelle `off_frimg` → 2712/2736/2760. `bi_pdata` blieb 3478/3606/3734.

**Fehlerfall ueber die dokumentierte API:** `rsrc_gaddr(R_IMAGEDATA, 0)` gibt den
BITBLK heraus, dessen `bi_pdata` eine kleine Ganzzahl ist — die Anwendung
zeichnet aus Adresse 3478.

## Was ich geaendert habe

Alles in `tos-api/src/aes/resource.rs`:

* **`relocate_blob_core` (LP64) und `relocate_blob_core_ilp32`**: der
  `match`-Arm-Block fuer `G_TEXT|G_BOXTEXT|G_FTEXT|G_FBOXTEXT`, `G_IMAGE` und
  `G_ICON` ist weg. Uebrig bleibt EmuTOS' eigene Unterscheidung: BOX-Typen
  (literales Farbwort), `G_CICON` (Tabellenindex), alles andere `+= tb`.
  Danach der Reihenlauf ueber `n_ted`/`n_ib`/`n_bb`, gefolgt von den
  frstr/frimg-Tabellen.
* **Neu `fix_nptrs64` / `fix_nptrs32`** — EmuTOS' `fix_nptrs`, ueber
  `(base, count, stride, field)`. Beide lesen und schreiben **unaligned**, damit
  keine Reihenbasis 8-ausgerichtet sein muss; die frstr/frimg-Tabellen benutzen
  dieselbe Funktion (`reloc_table_u64`/`reloc32` sind aufgegangen).
  `SUBPTR_FIELDS = 3` benennt, dass TEDINFO und ICONBLK ihre drei Zeiger als
  erste Glieder tragen — genau das, was `get_addr` als `0`/`ps`/`2*ps` rechnet.
* **`fix_tedinfo_std` / `fix_tedinfo_std_ilp32`** laufen ueber die
  TEDINFO-**Reihe** (`n_ted`) statt ueber die Objekte; `has_tedinfo` entfaellt.
  Siehe „Verhaeltnis zu A37".
* **`substruct_seen` / `substruct_seen_ilp32` entfallen.** Die Reihe besucht
  jeden Satz genau einmal, eine Entdopplung ist begrifflich nicht mehr moeglich.
  Neuer Dauertest `a_shared_tedinfo_is_relocated_exactly_once` haelt das fest.
* **Beibehalten, ausdruecklich:** die Schranke, dass ein `ob_spec` mit
  Substruktur wholly im Blob liegen muss (`record_fits`, jetzt aus der neuen
  `substruct_bytes` gespeist). Sie kam vorher als Nebenprodukt daher, weil durch
  `ob_spec` geschrieben wurde; ohne sie waere mit der Umstellung **still** eine
  Speicherschranke verschwunden, denn `objc_draw` dereferenziert `ob_spec` als
  TEDINFO/ICONBLK/BITBLK. EmuTOS hat sie nicht, weil es keinen Speicherschutz
  zu wahren hat — das steht so im Code.
* **Neu: ein NRSC aelter als v5 mit Substruktur-Objekt wird abgelehnt.** Ein
  solcher Kopf deklariert die drei Reihen nicht, also gibt es nichts, das man
  nach Zahl durchlaufen koennte. Ihm `ob_spec` folgen hiesse den Defekt
  wiederherstellen; ihn mit rohen Offsets herausgeben waere ein wilder Zeiger.
  Also `rs_readit`s eigene Antwort auf eine Ressource, die es nicht lesen kann:
  FALSE. Ein v1-v4-Blob **ohne** solches Objekt laedt unveraendert (der
  `selftest_objc`-Blob ist genau das).

## Deckungsnachweis: der Reihenlauf verliert nichts

Die eine echte Regressionsgefahr der Umstellung ist ein Satz, den der Objektlauf
erreichte, die Reihe aber nicht — also ein `ob_spec`, das **neben** die
deklarierte Reihe zeigt. Selbst nachgerechnet ueber alle drei ausgelieferten
Blobs (eigener Parser ueber den v5-Kopf, jedes Objekt der Substruktur-Typen
gegen `[base, base + n*stride)` und auf Vielfaches des Strides geprueft):

| Datei | `n_ted`/`n_ib`/`n_bb` | referenzierte Saetze | ausserhalb der Reihe |
|---|---|---|---|
| `tos-api/src/aes/gem_fsel.nrsc` | 13 / 0 / 3 | 13 / 0 / **0** | **keine** |
| `sdk/apps/qed/build/qed.rsc` | 118 / 1 / 7 | 118 / 1 / 7 | **keine** |
| `sdk/apps/teradesk/build/dist/desktop.rsc` | 104 / 0 / 0 | 104 / 0 / 0 | **keine** |

Der Reihenlauf deckt den Objektlauf also vollstaendig ab und fuegt in
`gem_fsel.nrsc` die drei Freibild-BITBLKs hinzu. Auch die drei
`frimg`-Eintraege zeigen exakt auf Reihenindizes der BITBLK-Reihe.

## Verhaeltnis zu A37 — die einzige Stelle, an der ich eine EmuTOS-Funktion teile

EmuTOS' `fix_tedinfo_std` (`:655-669`) tut **beides**: es relokiert
`te_ptext`/`te_ptmplt`/`te_pvalid` und rechnet die zwei Laengenwoerter neu. Es
laeuft nur im `rs_readit`-Pfad; die eingebaute AES-Ressource kommt ueber
`gem_rsc_init` und braucht weder das eine noch das andere (`erd` hat sie schon
als Zeiger und mit richtigen Laengen abgelegt).

Bei uns ist die eingebettete GEM.RSC **auch eine umgewandelte Datei** und
braucht die Zeiger-Haelfte. Deshalb ist geteilt:

* **Zeiger-Haelfte → `relocate_blob_core`s Reihenlauf**, den beide Einstiege
  fahren (`relocate_blob` fuer die eingebettete GEM.RSC, `relocate_and_register`
  fuer `rsrc_load`).
* **Laengen-Haelfte → `fix_tedinfo_std`**, aufgerufen nur aus
  `relocate_and_register_core`, also nur im `rs_readit`-Pfad.

Das beobachtbare Ergebnis ist damit EmuTOS' eigenes; `aes_rsrc_tedinfo_reserve.rs`
(A37s zweite Haelfte, `te_txtlen` 39 ueber leerem Text auf dem eingebetteten
Pfad) bleibt gruen. **A37 wird nicht doppelt:** sein Nachrechenschritt ist nicht
redundant geworden, er ist umgezogen — und erreicht jetzt auch ein TEDINFO, das
kein Objekt referenziert (neuer Test
`an_unreferenced_tedinfo_gets_its_lengths_recomputed`).

## Der ILP32-Zwilling — was ich vorgefunden habe

`Abi::Ilp32` ist zur Laufzeit **nicht erreichbar**: die einzige Stelle, die die
ABI eines Prozesses setzt, ist `loader.rs:589` `proc_slot.set_abi(Abi::Lp64)`,
davor weist `:413` jedes ELFCLASS32-Abbild mit `LoadError::Ilp32Unsupported`
ab (`tos-api/tests/loader_ilp32_rejected.rs`). Kein Hosttest ruft die
ILP32-Relokation an; `relocate_and_register` ist fest LP64.

Ich habe den Zwilling **mitgeaendert** statt ihn stehen zu lassen. Grund: die
Alternative waere, dass der ILP32-Pfad als einziger den objektgetriebenen
Defekt behaelt und der m68k-Thunk ihn spaeter erbt — genau das Auseinanderlaufen,
das die Zwillingsstruktur vermeiden soll. Ehrlich benannt: die ILP32-Aenderung
ist **nur uebersetzt, nicht ausgefuehrt**. Das gilt unveraendert aus A37.

## Test vor Fix — die rote Ausgabe

`tos-api/tests/aes_rsrc_substruct_arrays.rs`, 6 Faelle, **4 vor dem Fix rot**:

```
running 6 tests
test a_free_image_yields_a_dereferenceable_raster ... FAILED
test a_shared_tedinfo_is_relocated_exactly_once ... ok
test an_unreferenced_tedinfo_gets_its_lengths_recomputed ... FAILED
test every_bitblk_pdata_is_relocated ... FAILED
test every_tedinfo_pointer_is_relocated ... ok
test unreferenced_substructures_are_relocated_too ... FAILED

---- every_bitblk_pdata_is_relocated stdout ----
bitblk[0].bi_pdata = 3478 is not a pointer into the blob
(base 49941774336, total_size 4975); the file offset was never rebased

---- a_free_image_yields_a_dereferenceable_raster stdout ----
frimg[0]->bi_pdata = 3478 is not a pointer into the blob …

---- unreferenced_substructures_are_relocated_too stdout ----
unreferenced tedinfo[0].te_ptext = 320 is not a pointer into the blob
(base 4374312432, total_size 512) …

test result: FAILED. 2 passed; 4 failed
```

Die erste Fassung des unreferenzierten TEDINFO-Tests hat den rohen Offset
**dereferenziert** und die ganze Testbinaerdatei mit SIGSEGV abgeraeumt; der
Zeigertest steht jetzt **vor** dem `strlen`, damit der Defekt gemeldet statt
gestorben wird.

Nach dem Fix:

```
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored
```

Die zwei, die schon vorher gruen waren, sind Waechter: alle 13 TEDINFOs von
`gem_fsel.nrsc` sind referenziert (da aendert sich nichts), und die
Einmal-Relokation eines geteilten Satzes hielt die alte Entdopplung ebenfalls
ein — der Test bewacht, dass die Umstellung sie nicht gebrochen hat.

## Belege

* `cargo run -p xtask -- test`: **492 → 499 gruen / 0 rot / 4 ignoriert**
  (+6 A36-Faelle, +1 A35-Zustandstest; die 3 neuen Ignorierten sind A35).
  `cargo test -p tos-api` allein: 489 gruen / 0 rot / 4 ignoriert; ohne meine
  beiden neuen Dateien 482/0/1 — die vom Auftrag genannte 492 ist die
  `xtask test`-Zahl, nicht die von `cargo test -p tos-api`.
* `cargo xtask build --board rpi4 --release` und `--board qemu-virt --release`,
  je frisch (vorher `kernel/src/lib.rs` **und** `tos-api/src/aes/resource.rs`
  angefasst, damit das geteilte Zielverzeichnis nicht „up to date" melden kann):
  beide Exit 0, je `tos-api` 64 + `kernel` 94 Warnungen.
* Clippy `cargo clippy -p tos-api --all-targets`: **94 Warnungen mit und ohne
  meine beiden neuen Testdateien** (Delta 0). Keine davon zeigt auf
  `resource.rs` ausser den fuenf bestehenden `Aespb is more private`; die fuenf
  `collapsible_if` liegen alle in `object.rs`, die zwei `never read` in
  `form.rs`/`window.rs` — alle vorbestehend, nicht angefasst.
* Rauchprobe QEMU `raspi4b` headless, Release, ohne Karte: `[desk] desktop up
  (0 drives)`, **0** Panik/Ausnahme im seriellen Mitschnitt; `pgrep` danach
  leer. Das ist ein schwaches Positiv — der Dateiselektor laedt seine GEM.RSC
  erst beim ersten Gebrauch.

## Warum nichts committet ist

`tos-api/src/aes/resource.rs` traegt gegen `HEAD` (`70caee2`) bereits die
**unversionierte** Arbeit von A37 und A38 (`header_is_sound`, der `len`-Parameter,
`fix_tedinfo_std`, `sh_find` in `rsrc_load`); `tos-api/tests/` enthaelt zwoelf
unversionierte Testdateien fremder Tickets; `docs/STATUS.md` traegt die
M06/M07-Eintraege. Ein `git add` meiner Pfade wuerde diese Fremdarbeit in meinen
Commit einsammeln, und die dafuer vorgesehene `git stash`-Figur ist mir im
geteilten Arbeitsbaum untersagt. Die Arbeit liegt vollstaendig im Baum; der
STATUS.md-Eintrag ist additiv eingefuegt (die M06/M07-Eintraege nachweislich
unberuehrt). Das Landen entscheidet der Lead.

## Was ich nicht angefasst habe

* `tools/mkrsc.py` und alle Bauprodukte (`*.nrsc`, `*.rsc`) — die Umstellung ist
  rein ladeseitig, keine Datei aendert sich.
* `tos-api/src/aes/object.rs`, `fsel.rs`, `window.rs`, `shell.rs`,
  `bdos/**`, `vdi/**` — fremde unversionierte Nachbararbeit.
* A35 (`ap_ptree`/`ap_rscmem`/`ap_rsclen`) — eigene Quittung,
  `TASK/A35-STATUS-UNFINISHED.MD`.
* Kein bestehender Test musste geaendert werden.

## Offen

1. **Abnahme durch den Tester** (siehe „Uebergabe" im Entwicklerbericht):
   QEMU `raspi4b` Boot + Screendumps beider Boards, `aes-r3-wind-state` gegen
   `EMUTOS14-AES-r3.BIN` als Regressionsprobe, Dateiselektor-Dialog (zieht
   `relocate_blob` mit dem neuen Reihenlauf).
2. **Der ILP32-Zwilling bleibt unausgefuehrt** (unveraendert aus A37).
3. **Ein positiver Beleg fuer den Freibild-Pfad auf der Maschine fehlt** — kein
   ausgeliefertes Programm ruft heute `rsrc_gaddr(R_IMAGEDATA)`. Der Hosttest
   belegt ihn; eine Maschinenmessung braeuchte erst einen Fall dafuer.
