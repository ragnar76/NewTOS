# A19 — Quittung

**Status: ERLEDIGT · host- und maschinenbelegt · Abnahme durch den Tester
offen** · 2026-09-14 · Der AES-Dispatcher kopiert `int_in` und `addr_in`
einmal in eigene Puffer und klammert dabei auf `min(IN_LEN, I_SIZE)` bzw.
`min(AIN_LEN, AI_SIZE)`. Fünf neue Dauertests, vier davon vorher rot. Beide
Boards frisch gebaut, Warnungsgrundlinie unverändert. Commit `9524c42`
(Fix) und `6019eff` (Status/Ticket), Messung
`measurement-2026-09-14-a19/`.

## Die Reihenfolge, die das Ticket verlangt: erst die Durchsicht

**Ergebnis: der Defekt ist LATENT, nicht aktiv.** Kein Manager in
`tos-api/src/aes/` liest heute ein `int_in`-Wort oder einen
`addr_in`-Schlitz jenseits dessen, was eine konforme Bindung für seinen
Opcode deklariert.

Die vollständige Tabelle — **alle 79 Opcodes, die `crysbind` behandelt,
auch die sauberen** — steht in
[measurement-2026-09-14-a19/MANAGER-SURVEY.txt](../measurement-2026-09-14-a19/MANAGER-SURVEY.txt).
Methode: jeder `pb.intin(N)` / `pb.intin_ptr(N)` / `pb.addrin(N)` im Baum
maschinell erhoben (alle Indizes sind Literale, es gibt keinen gerechneten
zu übersehen), Hilfsfunktionen ihrem Aufrufer zugeschlagen
(`set_newdesk`/`wf_str_ptr` → `wind_set`, `do_chg` → `menu_text`, `run` →
`fsel_input`/`fsel_exinput`, `set_user_form` → `graf_watchbox`); Sollwerte
aus `references/emutos/desk/gembind.c` (EmuTOS' eigene Bindung,
`AES_CTRL_CODE(op, nintin, nintout, naddrin)`, Zeile je Opcode zitiert) und
für die neun Opcodes, die EmuTOS' Desktop nie ruft, aus
`sdk/gemlib/gemlib-0.44.0-m68k/a_*.c` (`AES_PARAMS`); gegengeprüft an den
`int_in[]`-Slotmakros in `include/aesdefs.h:161-609` und am jeweiligen
`crysbind`-Zweig (`aes/gemsuper.c:77-421`).

Zwei Dinge, die dabei auffielen und **kein** A19-Fall sind:

**Grenzfall, kein Befund — `wind_get(WF_COLOR)` / `wind_get(WF_DCOLOR)`.**
Unser `wind_get` liest `int_in[2]` (die Gadget-Nummer) in genau diesen
beiden Zweigen. Das ist exakt der Schlitz, den EmuTOS' eigenes `wm_get`
liest: `crysbind` reicht `&WM_IX` = `&int_in[2]` herein
(`gemsuper.c:342`), `wm_get` nimmt `pinwds[0]` für beide Felder
(`gemwmlib.c:1568`, `:1573`). gemlibs Bindung deklariert für genau diese
zwei Felder `IN_LEN 3` und für jedes andere `2` (`a_wind_get.c:336-348`) —
ein konformer Aufrufer deckt den Schlitz also. **EmuTOS' eigene
Desktop-Bindung deklariert dort unbedingt `2`** (`desk/gembind.c:757`) und
läse damit ihren uninitialisierten Stapel; das ist eine
EmuTOS-Inkonsistenz, nicht unsere, und weder EmuTOS' Desktop noch unser
`kernel/src/desk/mod.rs` fragt je nach einem der beiden Felder.

**Zwei Unterleser — eigener Befund, gemeldet, nicht angefasst.**
`appl_tplay` (14) und `appl_trecord` (15) lesen **gar kein** `int_in`,
obwohl die Bindung 2 bzw. 1 Wörter deklariert (`gembind.c:163`, `:172`;
EmuTOS' Zweig liest `AP_TLENGTH`/`AP_TSCALE`, `gemsuper.c:109-114`).
`shel_write` (121) liest `int_in[0..1]` und lässt `SH_ISCR = int_in[2]`
liegen, das die Bindung deklariert (`gembind.c:914`, `aesdefs.h:602`).
Das ist der **umgekehrte** Defekt und gehört nicht in diesen Fix — er
macht solche Stellen ohnehin nicht richtig, sondern nur harmlos.

## Der Fix

`tos-api/src/aes/mod.rs`:

* Neue Konstanten `I_SIZE = 16` (`include/aesdefs.h:122`) und
  `AI_SIZE = 3` (`:124`) neben dem vorhandenen `O_SIZE = 7`. Unser `O_SIZE`
  stimmte schon mit EmuTOS' überein; die anderen beiden gab es bei uns
  nicht, weil es keinen lokalen Puffer gab, dessen Größe sie hätten sein
  können.
* `Aespb::from_raw` ist jetzt `xif`s Marshalling-Schritt: es liest die
  Felder des Aufrufers **einmal**, `min(IN_LEN, I_SIZE)` Wörter und
  `min(AIN_LEN, AI_SIZE)` Schlitze, in dispatch-lokale Puffer, und die
  Felder `intin`/`addrin` der `Aespb` zeigen danach auf diese Puffer. Eine
  **negative** Länge kopiert nichts — EmuTOS' `if (IN_LEN)` ist dafür wahr
  und `min(IN_LEN,I_SIZE)*sizeof(WORD)` weitet sich zu einem `memcpy` über
  rund vier Gigabyte, kein Verhalten mit einem Wert; `copy_intout_back`
  liest ein negatives `OUT_LEN` seit jeher genauso.
* `dispatch` legt `local_intin[I_SIZE]` und `local_addrin[AI_SIZE]` neben
  das schon vorhandene `local_intout[O_SIZE]`. Alle drei genullt, aus dem
  Grund, der für `local_intout` schon im Quelltext stand: EmuTOS' `auto`-
  Speicher ist unbestimmt, es gibt **keinen Wert zu reproduzieren**, und
  ein definiertes 0 ist besser als fremde Bytes. (Der Unterschied zu
  EmuTOS ist damit benannt und nicht wegargumentiert: dort steht hinter der
  Grenze der vorige Stapelinhalt, bei uns 0. Beides ist unbestimmt; nur
  eines davon kann Anwendungsspeicher sein.)
* `intin()`, `intin_ptr()` und `addrin()` lesen die lokalen Puffer.
  `addrin()` bekommt die Zeiger bereits auf native Breite geweitet — die
  `Abi`-Fallunterscheidung ist von der Lesezeit auf die Kopierzeit
  gewandert, sonst nichts.
* `intin_ptr()` prüft zusätzlich, dass der gepackte Zeiger ganz in
  `I_SIZE` passt (LP64: vier Wörter). Bei `wind_set` beginnt er auf Wort 2
  und gemlib deklariert `num_intin = 6` (`a_wind_set_str.c:21`), also
  Wörter 2..5 — genau die sechs zugesagten.
* Neues Feld `caller_intin`, **nur** für den Trap-Tracer: dessen
  `NO_INTIN`-Flag heißt „der `intin`-Zeiger des Aufrufers war null" und
  wäre sonst still zu totem Code geworden, weil der lokale Puffer nie null
  ist. Ohne das `aes-trace`-Feature ist das Feld
  `#[cfg_attr(..., allow(dead_code))]`, damit die Warnungszahl nicht
  steigt.

`tos-api/src/aes/trace.rs`: die beiden Aufnahmestellen lesen `int_in` und
`addr_in[0]` jetzt aus den dispatch-lokalen Puffern, also aus dem, was der
Manager wirklich gesehen hat; `NO_INTIN` kommt aus `caller_intin`.

**Nicht angefasst, weil schon richtig: die Ausgabeseite.**
`copy_intout_back` klammert seit jeher auf `min(OUT_LEN, O_SIZE)` und die
Manager schreiben seit jeher in ein dispatch-lokales `int_out` — die
Klammer dort ist sogar strenger als EmuTOS' (das `OUT_LEN` nicht gegen
`O_SIZE` prüft), und das ist im Quelltext als bewusste Grenze begründet.
`addr_out` wird nur von `rsrc_gaddr` beschrieben, dem einzigen
`set_addrout`-Aufrufer, und `AO_SIZE` ist 1; EmuTOS kopiert dort ebenfalls
genau einen Schlitz und auch nur für diesen Opcode (`gemsuper.c:446-447`).

## Nachweis

**Test vor Fix, rot** (`tos-api/tests/aes_dispatch_inlen_clamp.rs`, fünf
Fälle, vier rot). Das Muster jenseits der Grenze ist absichtlich kein
Nullmuster: `0x1234, 0x5678, 0x4321, 0x0765` in `int_in[1..4]`, gelesen
über `wind_create` → `WS_FULL` → `wind_get(WF_FXYWH)`, und die Zeichenkette
`"C:\A19PAST.SCP"` hinter `addr_in[0]`.

```
wind_create_with_in_len_1_must_not_see_the_four_words_behind_it
  left: [4660, 22136, 17185, 1893]   right: [0, 0, 0, 0]
the_cut_is_exactly_at_in_len_not_one_word_early_or_late
  left: [4660, 22136, 17185, 1893]   right: [4660, 22136, 0, 0]
in_len_0_copies_nothing_at_all
  left: [4660, 22136, 17185, 1893]   right: [0, 0, 0, 0]
scrp_write_with_ain_len_0_must_not_see_the_pointer_behind_it
  left: 67 ('C')                     right: 0
```

Der fünfte Fall, `a_fully_declared_call_still_receives_every_word`, war
vorher wie nachher grün — er ist die Gegenprobe, dass die anderen vier an
der Grenze scheitern und nicht daran, dass `wind_create` sein Rechteck
nicht mehr ablegt. Er prüft auch die andere Hälfte von `min()`: `IN_LEN 30`
klammert auf `I_SIZE`, ohne den Aufruf zu verkürzen.

**Nach dem Fix grün:** `cargo run -p xtask -- test` → **545 passed / 0
failed / 1 ignored** (Sollstand 540 + die fünf neuen).
`measurement-2026-09-14-a19/host-test.log`.

**Beide Boards frisch gebaut** (`--release`, je ein `touch` auf
`tos-api/src/aes/mod.rs` davor, damit das geteilte `target/` kein „up to
date" behaupten kann):

| | sha256 |
|---|---|
| qemu-virt ELF | `23bbbe7a88c8ecbe70aac00f2ccc11d05cf8a7ab748de7885926d6f09a5186eb` |
| rpi4 ELF | `b8fc1da3833dc1cc023e5ad2c386c6e62e17367d534acb43216bba99d09f205e` |
| kernel8.img (rpi4) | `eeb6e505cb79ea4a596b36fb2d49ed3cc80b83e91887e2f175ee76a70d2eab12` |

Die beiden ELF-Hashes sind verschieden — das ist der Beleg, dass der zweite
Bau ein Bau war. Der rpi4-ELF und `kernel8.img` haben sich über zwei Bauten
bitgleich reproduziert; das ist der Beleg, dass die QEMU-Läufe unten dieses
Artefakt benutzt haben.

**Warnungen, Delta 0:** tos-api 64 / kernel 94 (rustc, beide Boards),
tos-api 81 / kernel 125 (clippy, Kernel auf beiden Boards geprüft). Keine
Clippy-Meldung hat ihren Hauptbereich in `aes/mod.rs` oder `aes/trace.rs`.
Logs im Messverzeichnis.

**Maschinennachweis, QEMU `raspi4b` headless, 640×480:**

* QED und TeraDesk per `#Z`-Autorun, QMP-Screendump 30 s nach Start:
  **0 von 307 200 Bildpunkten** unterscheiden sich gegen
  `measurement-2026-09-14-v17/qemu-tmp/` — beide Bilder bitgleich. Genau
  die Erwartung des Auftrags.
* `tosdump/cases/aes-m07-parameter.tsv` (20 Fälle, CRC `0F46665B`) — die
  Parametertabelle, die `OUT_LEN 0/1`, `IN_LEN 2` mit einem gesetzten Wert
  und `AIN_LEN 3` ohne Pflanzung ausdrücklich abklopft: das erzeugte
  `NEWTOS-AES-m07.BIN` ist **bytegleich** zu dem des V17-Laufs
  (`ca542485947fe92489784b87701110e74d3d36212f9b638e1443e6a4719c6f73`).
* `tosdump/cases/aes-r1.tsv` (95 Fälle): gegen den TT-Referenzdump
  **dasselbe Differenzprofil** wie am 2026-09-13, Feld für Feld und Zahl
  für Zahl (`global[2,3,4,10,11,12]` je 95×, `int_out[0]` 2×). Gegen den
  Lauf vom 2026-09-13 selbst verschiebt sich **ein einziges Feld**,
  `global[11]` von `0x4460` auf `0x4470` — das ist `ap_3resv` =
  `pd::table_addr()`, die Adresse eines Kernel-Statics, und sie stand im
  V17-Bau bereits auf `0x4470`. Also V17-Ära, nicht A19.
* Kein Panic, kein Fault, keine Exception; der Desktop kam hinter beiden
  Dumperläufen hoch. Keine QEMU-Prozesse hinterlassen.

**Ein Lauf der AES-Referenztabellen ändert sich nicht.** Die vier
eingefrorenen Tabellen fahren als Hostsuite mit (`aes_reference.rs`, 204 +
158 + 59 Fälle) und sind grün; die m07- und r1-Läufe oben sind dieselbe
Frage auf der Maschine, über den echten EL0-Programmpfad statt über
`dispatch` direkt. Keine gemessene Tabelle sieht die Klammer.

## Was offen bleibt

* **Abnahme durch den Tester.** Der behavioural PASS/FAIL ist seiner, nicht
  meiner; ich habe gebaut und die beiden QEMU-Gegenmessungen gefahren.
* **Echte Pi 4.** Nichts an dieser Änderung ist boardabhängig — es ist
  reines Parameter-Marshalling in `tos-api`, identischer Code auf beiden
  Boards — aber es lief keine Hardware und es wurde keine
  Hardware-Aktion ausgeführt.
* **Ein ILP32-Aufrufer.** Dumper und Hostharness sind beide LP64, also
  läuft der `Abi::Ilp32`-Zweig der `addr_in`-Weitung und von `intin_ptr`
  in keiner der Messungen. Es ist derselbe Zweig wie vorher —
  `abi.read_ptr` ist von der Lese- auf die Kopierzeit gewandert — aber
  gemessen ist er hier nicht.
* **Die zwei Unterleser** (`appl_tplay`/`appl_trecord`,
  `shel_write`/`SH_ISCR`) brauchen einen eigenen Auftrag.
