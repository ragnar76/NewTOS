# A50 — Quittung

**Status: DONE (Hostbeleg + QEMU-Smoke; Tester-Abnahme offen)** ·
2026-09-20 · uncommitted im Baum (Basis `5548536`). `TASK/A50.md`s Kopfzeile
bleibt unveraendert. Anwenderentscheid 2026-09-20: **Option (a)** — Rueckgaberegel
angleichen, Indexgrenze behalten und als benannte Abweichung eintragen; D-5 voll
angleichen.

## Was EmuTOS tut (gepinnte Quelle `1a7484ab`)

| Stelle | Befund |
|---|---|
| `aes/gemrslib.c:112-176` `get_addr` | je `R_*`-Typ Adressrechnung **ohne** Indexpruefung (`get_sub`, `:102-106`); `R_STRING`/`R_IMAGEDATA` liefern den **Inhalt** des Slots (`:159-162`) |
| `:170-172` | `default: return (void *)-1L;` — fasst `rs_hdr` nie an |
| `:715-721` `rs_gaddr` | `*rsaddr = get_addr(rtype, rindex); return (*rsaddr != (void *)-1L);` — Vergleich gegen **`-1`**, nicht `!= 0` |
| `:725-742` `rs_saddr` | `psubstruct = get_addr(...); if (psubstruct != (void **)-1L) { *psubstruct = rsaddr; return TRUE; } return FALSE;` |
| `:576-585` `fix_long` | `if (lngval != -1L) *plong = rs_hdr + lngval` — ein `0`-Offset wird bei EmuTOS `rs_hdr`, nur `-1` bleibt; ein Null-Slot entsteht dort ueber `rsrc_saddr(R_FRSTR, i, NULL)` (NRSC laesst `0` bei `0`, `fix_nptrs64`) |

Folgen: unbekannter Typ → `addr_out[0] = -1`, `int_out[0] = 0` (geladen oder
nicht); Null-Slot → `addr_out[0] = 0`, `int_out[0] = 1`; Index jenseits → `1`
mit Zeiger hinter die Tabelle; `rs_saddr` durch einen Null-Slot → Store nach
Adresse 0, `TRUE`.

## Einordnung der r4-Faelle (`docs/analysis/aes-r4-subfunktionen-einordnung-2026-09-12.md`)

* **D-5, Faelle 623–625** (Typ 17 / 99 / −1): jetzt **wortgleich** —
  `addr_out[0] = 0xFFFFFFFF` (ILP32; `Abi::write_ptr` schneidet `u64::MAX` auf
  4 Byte) bzw. alle Einsen (LP64), `int_out[0] = 0`, mit und ohne geladene
  Resource — der `default:`-Zweig liest keinen Header, bei uns liegt er
  deshalb **vor** dem A39-Selektor (`get_addr` auf `Loaded::EMPTY`).
* **D-4, die 25 Faelle 598–614, 616–621, 626, 627:** alle mit
  `rs_hdr == NULL` auf beiden Maschinen gemessen (D-4-Tabellenkopf: „`rs_hdr`
  ist auf beiden Maschinen NULL", EmuTOS rechnet aus der m68k-Vektortabelle) —
  das ist der Fall **„nichts geladen"**, kein Null-Slot-Fall. Sie bleiben
  Abweichung per A39-Vertrag (`0`/`0`) und sind im COMPATIBILITY-Eintrag als
  Fehlerfall 2 benannt; 626 (`R_OBJECT` 1000) ist zusaetzlich die Form von
  Fehlerfall 1 (Index jenseits). Keiner der 25 wird wortgleich.

## Aenderungen (uncommitted)

| Datei | Was |
|---|---|
| `tos-api/src/aes/resource.rs` | `enum Resolved { Slot, UnknownType, OutOfRange }`; `get_addr` → `Resolved` (`default:` → `UnknownType`, Grenze → `OutOfRange`); `deref_slot`: Null-Slot ist eine Antwort (`Slot { target: 0, access: 0 }`), kein Miss; neu `resolve()` = `rs_sglobe` + `get_addr`, „nichts geladen" = `get_addr` auf `Loaded::EMPTY`; `rsrc_gaddr`: `Slot → (target, 1)`, `UnknownType → (u64::MAX, 0)`, `OutOfRange → (0, 0)`; `rsrc_saddr`: schreibt nur bei `Slot` mit `target != 0`, sonst `FALSE`; `rsindex` als `UWORD` (`as u16`, −1 = 65535 wie EmuTOS); sechs In-Crate-Testaufrufe auf `.slot()` |
| `tos-api/tests/aes_rsrc_gaddr_edges.rs` | **neu**, 6 Tests, echter Dispatcher, handgebauter NRSC-v5-Blob mit `frstr[1] = NULL`, `frimg[0] = NULL` |
| `docs/api/aes/resource-fileselector.md` | §2 `rsrc_gaddr`/`rsrc_saddr`: die beiden `-1`-Folgen + NewTOS-Status-Callouts; Fehlertabelle: 6 Zeilen (unbekannter Typ gaddr/saddr, Null-Slot gaddr/saddr, Index jenseits, nichts geladen); A39-Callout Punkt 4 (unbekannter Typ liefert auch dort `-1`) |
| `docs/COMPATIBILITY.md` | neuer Eintrag „`rsrc_gaddr`/`rsrc_saddr` refuse an index past the table; EmuTOS hands out a pointer past it" (drei Fehlerfaelle ausgeschrieben, Begruendung EL0/EL1-Grenze, was **nicht** abweicht) |

## Test vor Fix / nach Fix

`cargo test -p tos-api --test aes_rsrc_gaddr_edges`

* **rot vor** (Produktcode unveraendert): 3 rot / 3 gruen —
  `an_unknown_type_answers_minus_one_and_false` (`left: (0, 0)`, `right: (0, 18446744073709551615)`),
  `a_null_slot_answers_address_zero_and_true` (`left: (0, 0)`, `right: (1, 0)`),
  `saddr_through_a_null_slot_answers_false_and_stores_nothing` (Vorbedingung `(1, 0)` verfehlt);
  gruen vor: Index jenseits (c), nichts geladen (d), `rsrc_saddr` unbekannter Typ (e).
* **gruen nach:** 6 / 6.
* `cargo run -p xtask -- test`: **666 / 0 / 1** (Baseline 660 + 6 A50). Alle
  `aes_rsrc_*`-Tests gruen ohne Umstellung (keiner prueft `ret 0` bei Null-Slot).

## Build / Clippy

Boards rpi4 / qemu-virt (`cargo clean -p kernel -p tos-api`, dann
`cargo xtask build --board …`): rustc **64/94**, Clippy **81/125** — Delta 0.
Die fuenf `resource.rs`-Treffer im Clippy-Log sind die vorbestehenden
`private_interfaces` auf `rsrc_load`…`rsrc_obfix` (`&Aespb`). Testdatei
clippy-sauber (`cargo clippy -p tos-api --test aes_rsrc_gaddr_edges`: kein
Treffer in der Datei).

## QEMU-Smoke (raspi4b, headless, Release `--board rpi4`, QMP-Screendump)

Image `tools/make-sd-image.sh` (dist/ mit cicons.rsc) + `NEWDESK.INF` `#Z`
wie `measurement-2026-09-19-a41/part2/method/`; Marker `sh_ldapp`, Settle 15 s.

* TeraDesk `#Z`: kein EXC/panic; `tera-up.png` **bytegleich** (SHA-256
  `5148fde2…`, 0 Pixel) zu `measurement-2026-09-19-a41/part2/screendumps/product1-tera-colour-icons-idle.png`.
* QED `#Z`: kein EXC/panic; `qed-up.png` **bytegleich** (SHA-256 `ec79c00a…`)
  zu `…/part2/screendumps/smoke-qed-up.png`.

Scratch: `$CLAUDE_JOB_DIR/tmp` bzw. `…/scratchpad/a50/` (Kernel, Images, Dumps,
Serial-Logs, Build-/Clippy-Logs). Keine Hardware-/Outward-Aktion, kein Commit.

## Verifikations-Handoff (Tester)

* Hosttest `tos-api/tests/aes_rsrc_gaddr_edges.rs` (6) — der eigentliche Beleg.
* QEMU beide Boards: Boot + `svc`-Selftests + Screendumps; TeraDesk `#Z`
  (prueft `rsrc_gaddr(R_TREE, 0)` auf `cicons.rsc` — bleibt 1) und QED `#Z`
  muessen bytegleich zu `measurement-2026-09-19-a41/part2/screendumps/` bleiben.
* Kein neuer Selftest verdrahtet; nichts ist Pi4-only.

## Offen / ausserhalb des Auftrags (nicht angefasst)

- `docs/STATUS.md` nicht angefasst (Auftragsscope); Zeile fuer A50 und den
  Recent-Decisions-Eintrag stehen aus.
- `docs/api/aes/_full-function-inventory.md` A-4 (Tabelle Teil II, Zeile
  „sonst … `None` → `0`" und der A-4-Block) beschreibt den Stand vor A50.
- `TASK/A50.md` Kopfzeile unveraendert (`OFFEN`), wie verlangt.
- Spec §„NewTOS-Status" zu D-054 (Ende der Datei) spricht noch von
  „still-unimplemented type codes 2–4/7–14" — historisch, seit A53 ueberholt.
