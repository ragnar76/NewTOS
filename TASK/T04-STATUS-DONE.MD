# T04 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Sichtabnahme offen (TeraDesk-Union)

Der Vertrag fuer Werte breiter als ein WORD in den AES/VDI-Wortarrays ist
geschrieben, gemlibs `*_long`-Accessoren sind auf LP64 32-Bit-Zugriffe, die
sechs Zaehl-Stellen deklarieren die Zeigerbreite des Aufrufers, und
`wind_get_ptr(WF_SCREEN)` liest den LONG statt acht Byte. Bearbeitet
2026-09-21, Branch `desk-rebuild`, Basis HEAD `b104beb` (der Auditor-Schritt 1
mit dem gruenen Vertragstest ist dort committet), **nicht committet, nichts
geflasht**. Lead-Entscheide: Regel = Vertrag; `WF_SCREEN` bleibt LONG-Klasse;
`vex_*` → eigenes Ticket T08; TeraDesk-Union nur beobachten.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemsuper.c:143` `ev_timer(MAKE_ULONG(T_HICOUNT, T_LOCOUNT))` mit `T_LOCOUNT = int_in[0]` (`include/aesdefs.h:190-191`; `MAKE_ULONG` `portab.h:139`); `aes/gemwmlib.c:1564` `gsx_mret((LONG *)poutwds, (LONG *)(poutwds+2))` = `*(LONG *)&int_out[1]`, `&int_out[3]`; `LONG` 32 Bit (`portab.h:123`); Zeiger `*(char **)pinwds` bei `pinwds = &int_in[2]` (`gemsuper.c:346`, `aesdefs.h:504`, `gemwmlib.c:1661,1687`) |
| NewTOS Kern | `tos-api/src/aes/event.rs` (`evnt_timer`/`evnt_multi`: `lo = int_in[n]`, `hi = int_in[n+1]`), `tos-api/src/aes/mod.rs::set_intout_long` (ein 32-Bit-Store je LONG), `Aespb::intin_ptr`/`ptr_words` (Zeiger in Aufruferbreite) — **0 Verstoesse** (Audit §2, `aes_t04_intin_long_contract.rs` 3/3) |
| gemlib (vorher) | `gem_aesP.h:89-99`, `gem_vdiP.h:182-185` `*(long *)(array + n)` = 8 Byte auf LP64; `n_v_delete_ctab.c:25` u. a. `0,2` statt `N_PTRINTS`; `a_wind_get.c:345`/`a_wind_sget.c:21` `4` statt `2 + N_PTRINTS`; `a_wind_get_ptr.c:51` `aes_intout_ptr(1, void *)` = 8 Byte ueber beide LONGs |
| gemlib (nachher) | `sdk/gemlib/overlay/gem_aesP.h`, `gem_vdiP.h`, `n_v_delete_ctab.c`, `n_v_delete_itab.c`, `n_vqt_fontheader.c`, `u_vqt_fontheader.c`, `a_wind_get.c`, `a_wind_sget.c`, `a_wind_get_ptr.c` |

## Was gebaut ist

* **Docs (Schritt 2).** `docs/COMPATIBILITY.md`: neuer Eintrag „A `LONG` in
  `int_in`/`int_out`/`ptsin`/`ptsout` is 32 bits over two WORDs …" (Form der
  A81/A57-Eintraege, EmuTOS-Zitate, die zwei Folgen: LP64-Accessoren muessen
  32 Bit sein; eine 8-Byte-Lesung eines 32-Bit-Stores ist falsch; Abgrenzung
  `vex_*` → T08, TeraDesk-Union → Pi-Beobachtung). `docs/api/aes/_dispatcher.md`
  §4 und `docs/api/vdi/_dispatcher.md` „NewTOS implementation notes": je ein
  Absatz mit Querverweis. `docs/sdk/05-abi-pitfalls.md` B16: „unresolved"
  aufgeloest (Regel, Header-Fix, die drei Folgen, Hinweis auf cflib
  `purec.c` und die TeraDesk-Union). `tos-api/src/aes/mod.rs`
  (`set_intout_long`-Doc-Kommentar): die cflib-Behauptung „get the value
  back" gestrichen — die 8-Byte-Lesung an `&int_out[1]` ergibt auf LP64
  `addr | len << 32`; `purec.c` ist ausserdem `__PUREC__`-gated.
* **gemlib-Overlay (Schritt 3).**
  * `gem_aesP.h`/`gem_vdiP.h`: unter `__NEWTOS__` `typedef int
    __attribute__((aligned(2), may_alias)) gem_wire_long` +
    `_Static_assert(sizeof == 4)`; `__aes_intin_long`/`__aes_intout_long`/
    `__vdi_array_long` liefern `gem_wire_long *` — Lvalue-faehig, also alle
    33 Schreiber (`aes_intin_long(n) = v`, `*__aes_intin_long(0, …) =
    errcode` in `x_form_xerr.c`) und 36 Leser samt `vdi_userdef.h`
    (`#undef`/redefine ueber dieselbe Inline-Funktion) ohne Dateiaenderung.
    `N_PTRINTS` auch in `gem_aesP.h` (fuer `a_wind_get.c`/`a_wind_sget.c`).
    Gewaehlte Form: `aligned(2)`-Typedef (nicht `memcpy` — das braeche die
    Lvalue-Verwendung; nicht `packed`-Struct — gleicher Code, mehr Text);
    Ziel-Disassembly geprueft (unten). Zeiger-Accessoren unveraendert
    (Aufruferbreite, A81). Der m68k-Bau (`build-gemlib-m68k.sh`) wendet das
    Overlay nicht an.
  * Klasse 2: `n_v_delete_ctab.c`, `n_v_delete_itab.c`, `n_vqt_fontheader.c`,
    `u_vqt_fontheader.c` → `cntrl_3 = N_PTRINTS`. Klasse 3c:
    `a_wind_get.c` (WF_NAME/WF_INFO `IN_LEN = 2 + N_PTRINTS`), `a_wind_sget.c`
    (`AES_PARAMS(104, 2 + N_PTRINTS, 1,0,0)`), `a_wind_get_ptr.c`
    (`What == WF_SCREEN` → `(void *)(unsigned long)(unsigned int)
    aes_intout_long(1)`, sonst `aes_intout_ptr(1, void *)`). Jede Datei mit
    dem Kopfmuster von `a_evnt_timer.c`, pristiner Rest byte-gleich
    (`diff` gegen den Tarball: nur Kopf + die eine Zeile).
* **Nicht gebaut — mit Beleg:** ein cflib-Overlay `purec.c` (Lead-Liste B).
  `sdk/cflib/cflib-21/purec.c` steht komplett unter `#if defined(__PUREC__)
  && !defined(_GEMLIB_COMPATIBLE)`; unter den NewTOS-Flags bleibt vom
  Praeprozessorlauf nur der gemlib-Prototyp `mt_wind_get` uebrig
  (`clang -E … | grep wind_get`: 4 Header-Prototypen, 0 Zeilen aus
  `purec.c`), `build/obj/purec.o` hat 720 Byte und keine Symbole, `cflib.a`
  kein `wind_get`. cflibs `wind_get` **ist** gemlibs `mt_wind_get`
  (`gem.h`-Makro). Ein Overlay wuerde toten Code aendern — nicht gemacht,
  hier und in `05-abi-pitfalls.md`/`COMPATIBILITY.md` vermerkt. Der
  Auditor-Eintrag „cflib `purec.c` CONFIRMED (Draht)" ist damit auf „nicht
  uebersetzt" zu korrigieren (Analyse-Datei unveraendert gelassen).
* **`TASK/T08.md`** (neu, P4): `vex_*`-Vektortausch in `CONTRL[7..]/[9..]`
  (Audit §6) — Kern liest/schreibt fest 8 Byte an festem Index, gemlib
  skaliert mit `N_PTRINTS`, EmuTOS zwei 32-Bit-Slots; Tabelle je ABI,
  Arbeit, Abnahme.
* **Tests:** `sdk/gemlib/tests/intin_long_words.c` (neu) — Fake-`vdi()`/
  `aes()`, prueft fuer `vst_arbpt32`, `v_create_ctab`, `vst_track_offset`,
  `v_value2color`: `control[0]` = Opcode, `control[3]`, exakt `[lo, hi]`
  an den Slots, Rueckgaben als 32 Bit sign-extended (`-2`), und fuer
  `mt_wind_get_ptr(WF_SCREEN)`: Adresse = der untere 32-Bit-Store (Fake fuellt
  `int_out[1..4]` wie der Kern, einmal mit High-Wort der Laenge ≠ 0),
  `WF_NEWDESK` in Aufruferbreite unveraendert; mit `-fsanitize=address`
  ueberwacht die exakt dimensionierten Arrays. `ptr_word_count.c` (neu) —
  die sechs Zaehl-Overlays (`control[3]`/`control[1]`, Zeigerbytes).
  Bauzeilen in den Dateikoepfen; Negativkontrolle = pristine
  `gem_{aes,vdi}P.h` aus dem Tarball vorn im Include-Pfad + pristine `.c`,
  `-D__NO_INLINE__` statt `-D__NEWTOS__`.

## Tests: rot vor, gruen nach

| Lauf | pristine (Tarball) | Overlay |
|---|---|---|
| `intin_long_words`, ASan | `stack-buffer-overflow WRITE of size 8 … vst_arbpt32` (Abbruch, rc 134); mit `-fsanitize-recover`: WRITE+READ 8 in `vst_arbpt32` (`vdi_intout[2]`), WRITE 8 in `v_value2color` (`vdi_intin[2]`); FAIL `return(-2) got 0xfffffffe`, `v_value2color return got 0x64000000010002`, `WF_SCREEN *v got 0xa00001000218` und `0x3f4800000fff0` (rc 1) | **OK**, rc 0, ASan still |
| `ptr_word_count` | 7 FAIL: `control[3] got 0x2 want 0x4` ×4, `control[1] got 0x4 want 0x6` ×3 (rc 1) | **OK**, rc 0 |
| `evnt_timer_words` (H03), `wind_set_addrin` (A81) mit den geaenderten Headern | — | OK, OK |
| `tos-api/tests/aes_t04_intin_long_contract.rs` | — | 3/3 (unveraendert, `b104beb`) |
| `cargo run -p xtask -- test` | — | **906 passed / 0 failed / 1 ignored** (903 + 3 Vertragstests von `b104beb`) |

Ausgaben: `measurements/measurement-2026-09-21-t04/host-tests/gemlib-host-tests-{GREEN-overlay,RED-pristine}.txt`, `xtask-test-AFTER.log`.

Im Ziel-Objekt (`-O2`, exakt die Flags von `build-gemlib.sh`) war das
`control[0]`-Ueberschreiben des Auditors real; im Host-ASan-Lauf (`-O1`,
anderer Rahmen) bleibt der Opcode erhalten und der Ueberlauf wird von ASan
statt vom Opcode gemeldet — deshalb beide Belege.

## Bibliothekskette und Disassembly

| Schritt | Ergebnis |
|---|---|
| `sdk/gemlib/build-gemlib.sh` | `compiled OK=578 skipped=0 (of 578)`, `failures match gemlib/expected-failures.txt: 0 today, 0 recorded, 0 new`, `LINK OK gemtest.NCP`; 0 Warnungen in den `.err` |
| `sdk/cflib/build-cflib.sh` | `129/129`, `failures match … 0 new`, `LINK OK cftest.NCP` |
| `sdk/apps/build-qed.sh` | `37/37`, `LINK OK QED.NAP`, `undefined symbols … 0` |
| `sdk/apps/build-teradesk.sh` | `48/48`, `LINK OK DESKTOP.NAP`, `undefined symbols … 0` |

Disassembly (`measurements/measurement-2026-09-21-t04/build/disassembly-evidence.txt`):
`vst_arbpt32` `stp wzr, w8, [x29,#-0x2c]` (32-Bit-Nullstore in `vdi_intout`
+ `control[0] = 246` in einem Paar), Rueckgabe `ldursw x9, [x29,#-0x2c]`;
`v_create_ctab` `stp w1, w2, [x29,#-0x28]` (zwei 32-Bit-LONGs), `control[0]
= 206` bleibt; `vst_track_offset` `sturh 255`, `sturh pairmode`, `stp w1, w8`
(LONG + Opcode 237); `v_value2color` `str w1, [x29,#0x1c]` innerhalb des
Rahmens; `mt_x_malloc` `stur w1`; `mt_wind_get_ptr` `cmp w21,#0x11` →
`ldur w8,[x20,#0x2]` (32 Bit an `&int_out[1]`), sonst `ldur x8`. Im gelinkten
**QED** (`mt_fnts_do`): `stur w2/w3/w4` an `intin[1/3/5]`, `ldursw` aus
`intout[2/4/6]`. Im gelinkten **TeraDesk** ist `a_wind_get.o` das einzige
geaenderte Mitglied (`why.txt`): `mt_wind_get` `mov w9,#0x6` +
`stur x22,[x8,#0x24]` (8-Byte-Zeiger an `int_in[2]`, `IN_LEN` 6) —
`mt_wind_get_ptr` wird von keinem Port gelinkt, deshalb das Objekt aus
`gemlib.a` als Beleg.

## Build / Clippy

`cargo xtask build --board rpi4`: tos-api **64** / kernel **94** Warnungen
(Basis 64/94, Delta 0; nur der Doc-Kommentar in `mod.rs` geaendert);
`--board qemu-virt`: kernel 94, tos-api 64 (Cache). `cargo clippy -p tos-api`:
**80 → 80**. Der Baum-`kernel8.img` ist der rpi4-Bau (bcm2711-Pfade, keine
`qemu_virt`). Logs in `measurements/measurement-2026-09-21-t04/build/`.

## QEMU raspi4b (`measurements/measurement-2026-09-21-t04/`)

Methode: A79/A100-Injektor in einer Scratch-Kopie (`method/make_scratch_t04.sh`,
kein Produktcode ersetzt — T04 aendert keinen Kernelcode), Skript
`method/tera-t04.actions`: parkbr; arm; **A** idle; File-Titel (88,8) → **B**;
gleiten auf View (135,8) → **C**; Options (260,8) → **D**; raus + Klick;
parkbr → **E**. Ein Scratch-Kernel fuer beide Laeufe; die Bilder
unterscheiden sich nur im `DESKTOP.NAP`: **nachher** = `build/dist` (T04),
**vorher** = rekonstituiert (`method/reconstitute-before.sh`: TeraDesks
Objekte + `gemlib.a`-Kopie mit pristinem `a_wind_get.o` aus HEAD-Headern;
Disassembly-Differenz zum T04-Binary **genau eine Instruktion**, `mov
w9,#4 → #6`). `#Z`-`NEWDESK.INF` + `TERADESK.INF` aus a79/method.

| Dump | vorher vs nachher | gegen a100-a102 `tera-after-*` |
|---|---|---|
| A idle | 0 px | 0 px |
| B File offen | 0 px | 0 px |
| C View offen | 0 px | — |
| D Options offen | 0 px | — |
| E geschlossen | 0 px | — |

0 `EXC`/`PANIC` in beiden Serial-Logs. Erwartet: TeraDesk erreicht den
einzigen geaenderten Pfad (`wind_get(WF_NAME/WF_INFO)`) nie.

**Die TeraDesk-Union, gelesen** (`sdk/apps/teradesk/resource.c:233-241,307,315`):
`wind_get(0, WF_SCREEN, &dummy, &dummy, &buffer.words.high, &buffer.words.low)`
— gemlibs `mt_wind_get` kopiert vier Worte: `words.high` (Offset 0) ←
`int_out[3]` = Bits 0..15 der Laenge, `words.low` (Offset 2) ← `int_out[4]` =
Bits 16..31; auf LE ergibt das in `long size` die **richtigen unteren 32
Bit** (die m68k-Namen `high`/`low` sind auf LE vertauscht, die
Union-Ueberlagerung dreht das wieder um). Die **oberen 32 Bit** des 8-Byte-
`long` schreibt niemand — Stapelinhalt. `if (mnsize >= buffer.size)` mit
`mnsize` ≈ Groesse der View-/Options-Box (bei 640×480 einige zehn KB):
obere Haelfte **0** → Vergleich gegen die echte Laenge (A57: ein Schirm in
Geraetepixeln ≫ Box) → falsch → nichts geloescht; obere Haelfte **positiv** →
`size` ≥ 4 GiB → falsch; obere Haelfte **negativ** (Bit 63) → `size` < 0 →
wahr → `mn_del` ×13 (SEP1–9, `MDELETE`, `MFCOPY`, `MSHSIZ`, `MREVS`). Die
Loeschung kann auf NewTOS also **nur durch Stapelmuell** ausgeloest werden,
nie durch die Laenge selbst; T04 aendert daran nichts (vor und nach T04
identische vier Worte). In QEMU: File-Box mit `Delete...`, `Copy disk...`
und drei Trennlinien, View-Box mit `Show size`/`Reverse order` und drei
Trennlinien, Options-Box mit drei Trennlinien — **alle 13 vorhanden**, die
obere Haelfte war hier ≥ 0. Auf der Pi (1920×1080, anderer Stapelinhalt
moeglich) ist das zu beobachten: fehlen dort die Trennlinien und
`Delete...`, ist es diese Union, nicht die gemlib — dann waere ein App-Overlay
(Vergleich ueber einen 32-Bit-Typ) die Reparatur (Lead-Entscheid: jetzt kein
Overlay).

## Nicht angefasst / offen

* **Pi-Sichtabnahme** (Tester/Lead, nichts geflasht): TeraDesk-Start,
  File/View/Options-Menues vollstaendig (13 Eintraege, s. o.); Dateien:
  `sdk/apps/teradesk/build/dist/DESKTOP.NAP` (`7739a2db…`), `QED.NAP`
  (`2c50ae42…`), Kernel unveraendert im Verhalten.
* `TASK/T08.md` (`vex_*`, P4) — offen.
* Auditor-Analyse `docs/analysis/t04-lp64-intin-long-contract-2026-09-21.md`
  unveraendert gelassen; ihre cflib-`purec.c`-Zeile ist durch den
  Praeprozessorbefund oben ueberholt (nicht uebersetzt statt CONFIRMED).
* Out of scope, beobachtet: die gemlib-Hosttests haengen weiter in keinem
  Skript (T06); die `features.h`-Makro-Redefinition (eine Warnung je TU beim
  Host-Bau der Tests) ist vorbestehend.
