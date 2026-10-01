# V16 — Quittung

**Status: DONE** · 2026-09-12 · Der Defekt ist nachgeprueft, rot gezeigt,
repariert, gruen gezeigt und gegengemessen. Nichts committet.

## Der Befund, selbst nachgeprueft

Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(`git -C references/emutos log -1` bestaetigt: *"Update GitHub Actions to use
Node 24"*, 2026-04-14). Zielprofil Falcon030/TOS4. `vdi_asm.S` steht ausserhalb
jeder `#if CONF_WITH_*`-Klammer — der zitierte Block ist in **jedem** Bauprofil
aktiv (die einzige Praeprozessor-Verzweigung der Datei in diesem Bereich ist
`__mcoldfire__` im `_GSX_ENTRY`-Rumpf darunter, nicht im Trap-Kopf).

`references/emutos/vdi/vdi_asm.S:33-52`:

```
_vditrap:
        cmp.w   #0x73,d0                // VDI call?
        jne     no_vdi
        jsr     _GSX_ENTRY
        rte
no_vdi:
        cmp.w   #0xffff,d0              // query for jsr:able address?
        jeq     query
/*
 * Just like Atari TOS, we leave d0 unchanged, indicating the absence of
 * GDOS if this is called with -2 in the low-order word of d0.
 */
        rte
query:
        move.l  #_GSX_ENTRY,d0          // Return address of VDI dispatcher
        rte
```

Ein nacktes `rte`, kein Schreibzugriff auf `d0`.

NewTOS, `kernel/src/arch/aarch64/exceptions.rs:399` (Zeilennummer aus dem
Ticket **stimmt**, nachgeprueft vor der Aenderung):

```rust
_ => 0, // unknown selector → no-op (GEM trap is trusted)
```

Und was der Aufrufer daraus macht, `sdk/gemlib/overlay/_gc_vq_gdos.c:39`:

```c
return (short)x0 != -2;
```

Mit `x0 = 0` ist das **1 = „GDOS vorhanden"**. `vq_vgdos()`
(`_gc_vq_vgdos.c`) liefert entsprechend `0` statt `-2`.

Die Spec ist richtig und wurde **nicht** angefasst: `docs/api/vdi/_dispatcher.md`,
Abschnitt „`d0` special values (`vdi_asm.S:40-52`)" fuehrt die Tabelle
vollstaendig (`-1` → Adresse von `_GSX_ENTRY`, `-2` → `d0` unveraendert, sonst
`rte`). Keine Luecke gefunden.

## Test vor Fix — rot, dann gruen

Neue Datei `kernel/src/selftest_gemtrap.rs` (vorher auf Existenz geprueft),
fuenf Pruefungen ueber den **echten** `svc #2`-Pfad; `vq_gdos()`/`vq_vgdos()`
sind darin Wort fuer Wort die Arithmetik der beiden gemlib-Overlays, damit der
Nachweis ueber die dokumentierte API laeuft und nicht ueber ein Internum.

QEMU `raspi4b`, headless, serieller Mitschnitt
(`measurements/measurement-2026-09-12-v16/SELFTEST-GEMTRAP-before-fix.txt`):

```
[gemtrap]   selector -2 leaves x0 unchanged (-2): FAIL
[gemtrap]   selector 999 leaves x0 unchanged (999): FAIL
[gemtrap]   vq_gdos() reports no GDOS (0): FAIL
[gemtrap]   vq_vgdos() reports no GDOS (-2): FAIL
[gemtrap]   selector -1 answers 0 (no jsr-able address; ...): OK
[gemtrap] === FAILURES — see above ===
```

Nach dem Fix (`…/SELFTEST-GEMTRAP-after-fix.txt`), gleicher Aufbau:

```
[gemtrap]   selector -2 leaves x0 unchanged (-2): OK
[gemtrap]   selector 999 leaves x0 unchanged (999): OK
[gemtrap]   vq_gdos() reports no GDOS (0): OK
[gemtrap]   vq_vgdos() reports no GDOS (-2): OK
[gemtrap]   selector -1 answers 0 (no jsr-able address; ...): OK
[gemtrap] === all checks passed ===
```

Der Selftest ist wie seine 23 Geschwister **nicht** in den Boot verdrahtet; fuer
die beiden Laeufe wurde der Aufruf voruebergehend in `lib.rs` gesetzt und danach
**von Hand** wieder entfernt (kein `git checkout`/`restore`/`stash`). `lib.rs`
unterscheidet sich vom Ausgangsstand danach nur noch um die Zeile
`pub mod selftest_gemtrap;` (per `diff` gegen eine Handkopie belegt).

## Wie „Trapframe unberuehrt lassen" geloest ist — und warum so

Der Rueckgabeweg ist **gemeinsam**: der grosse `match class { … }` liefert ein
`result: i64`, und `complete_call_tail(frame, result)` schreibt es mit
`frame.gpr[0] = result as u64` in den Frame — fuer *jede* Trap-Klasse, und
ausserdem ein zweites Mal aus `handle_psigreturn` (der AES-Userdef-Abschluss),
damit beide Wege byteidentisch enden.

Zwei Wege waren moeglich:

1. **`result` zu `Option<i64>` machen** und den Schreibvorgang ueberspringen.
   Das ist die woertlichere Uebersetzung von `rte`, aendert aber die Signatur
   von `complete_call_tail` und damit den Rueckgabeweg **aller** Trap-Arme
   einschliesslich des zweiten Aufrufers — genau die Ausweitung, die der
   Auftrag ausschliesst.
2. **`x0` mit seinen eigenen Bits zurueckgeben.** Zwischen dem Eintritt in den
   Handler und dieser Stelle wird `frame.gpr[0]` auf diesem Pfad nirgends
   veraendert (die beiden frame-mutierenden Fruehausstiege — `pending_switch`
   und `pending_stall` — kehren vor dem Schreibvorgang zurueck, und die drei
   Tracer dazwischen fassen den Frame nicht an). `frame.gpr[0] as i64` und
   zurueck `as u64` ist ein verlustfreier Rundgang, der Schreibvorgang also
   bitgenau `x0 = x0`. Der Frame ist eine gewoehnliche Speicherstruktur, die
   der `eret` zurueckspielt — kein MMIO, kein `volatile` —, also gibt es
   keinen beobachtbaren Unterschied zum ausgelassenen Schreibvorgang.

Gewaehlt: **(2)**. Ein Arm geaendert, null Zeilen an den anderen Armen, null
Zeilen am gemeinsamen Rueckgabeweg. Der Kommentar an der Stelle sagt beides:
warum `x0` unveraendert bleiben muss (EmuTOS-Beleg) und warum das hier als
Rueckgabe und nicht als Auslassung geschrieben ist.

## F-2 (Selektor `-1`) — Entscheidung: **bleibt bei `0`, ist jetzt dokumentiert**

EmuTOS beantwortet `d0 = -1` mit `move.l #_GSX_ENTRY,d0` — der jsr-baren Adresse
des VDI-Dispatchers (`vdi_asm.S:51-52`). NewTOS hat keinen solchen
Einsprungpunkt: der VDI ist ausschliesslich ueber `svc #2` erreichbar, und der
Dispatcher ist EL1-Code, den ein EL0-Aufrufer weder anspringen noch lesen kann.
Es gibt also **keine Adresse, die die Frage wahrheitsgemaess beantworten
wuerde**; eine echte Kernadresse herauszugeben waere nutzlos *und* ein
Adressleck. `0` — „keine jsr-bare Adresse" — ist der Wert, auf den ein
konformer Aufrufer prueft, bevor er das Ergebnis benutzt, und der Wert, den
NewTOS hier schon immer geliefert hat.

**Wichtig, und deshalb angefasst:** die neue „`x0` unberuehrt"-Regel haette
`-1` geerbt und `-1` zurueckgegeben — einen Wert, der *wie* eine Adresse
aussieht. Der Fall steht deshalb jetzt als **eigener Trap-Arm**
(`GEM_SEL_ADDR_QUERY`) da und liefert weiter `0`. Das ist keine
Verhaltensaenderung gegenueber heute (die Sonde des Auditors mass `sel -1 →
x0=0x0`, unser roter Lauf ebenso: diese eine Pruefung war schon vor dem Fix
`OK`), sondern das, was den Wert gegen die Nachbarregel haelt.

Eingetragen in `docs/COMPATIBILITY.md` unter „Sanctioned deviations, one by
one" — benannt, datiert, mit Begruendung und mit dem ausdruecklichen Zusatz,
dass `-2` **keine** sanktionierte Abweichung ist, sondern in derselben
Aenderung repariert wurde.

## Bestehende Tests

**Keiner musste geaendert werden**, und keiner schrieb den falschen Zustand
fest. Geprueft: der ganze Baum kennt ausser den beiden gemlib-Overlays keinen
Aufrufer mit einem anderen Selektor als 115/200 (`grep -rn 'svc[[:space:]]*#2'`
ueber `kernel tos-api sdk tosdump tools`) — `kernel/src/vdi_client.rs:76`,
`selftest_vdi.rs:20`, `selftest_aes.rs:18`, `selftest_objc.rs:29`,
`desk/mod.rs:109`, `sdk/gemlib/overlay/_gc_vdi.c:35`, `tosdump/abi/newtos_svc.c`.
Die Host-Testsuite (`tos-api`, `tos-types`, `xtask`) erreicht den GEM-Trap
ueberhaupt nicht; der Kernel ist nicht host-linkbar (`xtask`-Hilfetext:
*"the kernel is not host-linkable"*), deshalb liegt der Dauertest als
`svc`-Selftest vor und nicht als `#[test]`.

## Mitgeaendert, weil meine Aenderung sie falsch macht

`sdk/gemlib/overlay/_gc_vq_gdos.c` und `_gc_vq_vgdos.c`: die beiden
NOTE-Absaetze behaupteten *"selector -2 is a no-op that leaves x0 = 0 … so this
reports GDOS present"*. Das ist nach dem Fix unwahr. **Nur die Kommentare**
sind neu geschrieben (mit EmuTOS-Beleg und dem Datum der Reparatur), kein Code —
die beiden Funktionen waren bereits richtig und rechnen unveraendert weiter.
`sdk/gemlib/overlay/_gc_vdi.c:19` ist **nicht** angefasst (Ticket A53).

## Abnahmestand

| Punkt | Stand |
|---|---|
| `cargo run -p xtask -- test` | **472 gruen / 0 rot / 1 ignoriert** — vorher **472 / 0 / 1**, unveraendert (der neue Test ist ein `svc`-Selftest, kein Host-Test) |
| `cargo run -p xtask -- build --board rpi4` | sauber, frisch kompiliert (`Compiling kernel v0.1.0`), `tos-api` 64 / `kernel` 94 Warnungen = Basisstand |
| `cargo run -p xtask -- build --board qemu-virt` | sauber, frisch kompiliert, dieselben Zahlen |
| `cargo clippy -p kernel --features board-rpi4` | `kernel` 125 / `tos-api` 81 Warnungen = Basisstand, **null** Treffer in `selftest_gemtrap.rs` oder `exceptions.rs` |
| `cargo clippy -p kernel --features board-qemu-virt` | identisch: 125 / 81, null Treffer in den beruehrten Dateien |
| `vq_gdos()` meldet kein GDOS | **ja**, `0` — gemessen ueber `svc #2` in QEMU `raspi4b` |
| `vq_vgdos()` liefert `-2` | **ja** — selber Lauf |
| Gegenmessung `vdi-r1` / `vdi-r2` / `vdi-r3` / `vdi-all` | **0 von 24 / 0 von 120 / 0 von 38 / 0 von 182** Faellen anders als der Vorstand vom selben Tag; gegen EmuTOS unveraendert 4 / 4 / 5 / 13 |
| Gegenmessung `aes-r3-wind-state` | **0 von 39** anders als der Vorstand; gegen EmuTOS weichen weiterhin **genau** 261, 280, 400, 481, 494, 574, 588 in `int_out` ab (Fallmenge nachgerechnet) |
| Bootgesundheit | alle fuenf Laeufe: `all N cases ran`, `tosdiff --validate` akzeptiert, **0** `[EXC]`/`PANIC`, `[desk] desktop up (1 drives)` |
| QEMU/Hatari-Prozesse | keiner laeuft (`pgrep -f 'qemu-system-aarch64\|Hatari'` nach jedem Lauf leer) |

Belege: `measurements/measurement-2026-09-12-v16/` (fuenf Laeufe mit `.BIN`/`.LOG`/
`BOOT-SERIAL.txt`/beiden Diffs, die zwei Selftest-Mitschnitte, `RUNINFO.TXT`).

## Nicht angefasst

`sdk/gemlib/overlay/_gc_vdi.c:19` (A53), `tos-api/src/aes/window.rs`
(A22/A33/A34), `tools/mkrsc.py`, `docs/api/aes/window-graphics.md`,
`docs/api/vdi/_dispatcher.md` (war richtig), alle Falltabellen unter
`tosdump/cases/`, alle `.BIN`/`.LOG` unter `tosdump/reference/`, alle
bestehenden `measurement-*`-Verzeichnisse. Keine Hardware beruehrt, nichts
ausgehaengt, nichts geflasht, nichts committet.

## Ausserhalb des Auftrags beobachtet, nicht verfolgt

* Der Selektorvergleich ist bei uns 64 Bit breit (`115`, `200`,
  `GEM_SEL_ADDR_QUERY`), bei EmuTOS ein reiner Wortvergleich (`cmp.w #0x73,d0`).
  Ein Aufrufer mit gesetztem oberen Langwort traefe daher bei uns den
  Unbekannt-Arm; kein Aufrufer im Baum tut das.
* `docs/api/vdi/_dispatcher.md` fuehrt in einem Blockzitat weiterhin die offene
  Frage „Selektor 115 vs. 200" (D-VDI-6) — laengst durch beide Zahlen im Code
  beantwortet, veraltete Protokollzeile.
* `docs/api/vdi/_working-list.md:139` (D-080) ist ebenfalls veraltet; steht
  bereits in A53.
