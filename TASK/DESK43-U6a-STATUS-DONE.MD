# DESK43 U6a — EL0-Spike `hello-gem` — Quittung

> Stand 2026-10-14 · Basis `main` @ `36745376` · Aenderung **uncommitted** im gemeinsamen Baum (nur neues Verzeichnis `programs/`)
> Auftrag: `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.2 (Zeile U6a), §5 (Werkzeugkette T1, vom Anwender freigegeben), §7.4 (Punkte 1, 3, 7)
> Messung: [`measurements/measurement-2026-10-14-desk43-u6a/`](../measurements/measurement-2026-10-14-desk43-u6a/RUNINFO.TXT)
> **Keine Kernel-Aenderung.** Nichts committet, keine SD-Karte, kein Pi; kein `git checkout/restore/clean/stash/reset`.
> `docs/STATUS.md` und `TASK/DESK43.md` unberuehrt.

Scope in einem Satz: ein minimales Rust-`no_std`-PIE-Programm `HELLOGEM.NAP`
auf einer kleinen Laufzeit-Crate bauen, mit `sdk/crt0.S` + `sdk/newtos.ld`
ueber `rust-lld` linken wie die C-Programme, und zeigen, dass der
unveraenderte Lader es annimmt und es als GEM-Anwendung in EL0 laeuft.

---

## 1. Ergebnis

**Das riskanteste Unbekannte des Neuschnitts ist geklaert: Rust-PIE trifft
den Lader-Vertrag ohne jede Kernel-Aenderung.** `llvm-readobj -d` zeigt
**genau die Marken von `QED.NAP`** (`FLAGS_1 DEBUG RELA RELASZ RELAENT
RELACOUNT SYMTAB SYMENT STRTAB STRSZ GNU_HASH HASH INIT/FINI_ARRAY(SZ)`),
alle auf der Positivliste (`tos-api/src/bdos/process/reloc.rs:78-116`);
Relokationen nur `R_AARCH64_RELATIVE` (28); `ET_DYN`, Einsprung `0x1000`,
dieselben vier Programmkopfzeilen wie jedes C-Programm.

QEMU raspi4b, memprot **on / off / strict**: Start per **`#Z`** und **vom
Desktop** (F1 auf installierter Anwendung) → Fenster mit Text, Taste beendet,
Desktop kommt sauber zurueck; Panic-Bau endet nur das Programm. **0 `[EXC]`,
0 `[PANIC]` in 15 Laeufen.** Alle Fenster-Screendumps sind ein und dasselbe
Bild; alle Desktop-Dumps danach sind byte-gleich mit dem sauberen Desktop
der U5a-Messung (`0e6f7704…324b`).

## 2. Was gebaut wurde

```
programs/                      eigener Workspace (Root: exclude = ["programs"])
├── Cargo.toml                 Profile: panic=abort, opt-level s, fat LTO, debug=false
├── Cargo.lock
├── .cargo/config.toml         Ziel aarch64-unknown-none-softfloat, -C relocation-model=pic,
│                              target-dir ../target/programs (im ignorierten /target)
├── build.sh                   crt0.o + cargo staticlib + rust-lld (Linkzeile von sdk/build.sh)
├── README.md
├── newtos-gem/src/            Laufzeit-Crate
│   ├── lib.rs                 #[global_allocator], #[panic_handler], FixedBuf (fmt ohne alloc)
│   ├── gemdos.rs              svc #1: Cconws, Malloc, Malloc(-1), Mfree, Pterm
│   ├── aes.rs                 svc #2/200: appl_init/exit, evnt_multi, form_alert, graf_handle,
│   │                          graf_mouse, wind_create/open/close/delete/get/set/update/calc
│   ├── vdi.rs                 svc #2/115: v_opnvwk/v_clsvwk, vs_clip, vswr_mode, vsf_*, vst_*,
│   │                          vr_recfl, v_gtext
│   └── heap.rs                GlobalAlloc ueber GEMDOS-Malloc-Hunks
└── hello-gem/src/lib.rs       HELLOGEM.NAP (staticlib, Feature panic-test)
```

* **Build-Einstieg:** Skript `programs/build.sh` statt `xtask build-programs`
  — am wenigsten invasiv (kein Eingriff in `tools/xtask`, dessen Hosttests
  zur Basislinie gehoeren); `xtask build-programs --board` mit Blob-Einbettung
  ist laut Schnitt U6b. Aufrufe:
  `programs/build.sh` → `target/programs/HELLOGEM.NAP`;
  `programs/build.sh hello-gem <out> -- --features panic-test`;
  `PRGFLAGS=…` wie bei `sdk/build.sh`.
* **Trap-Bindungen:** Muster aus `kernel/src/desk/{dosbind,vdibind}.rs` und
  dem AES-Teil von `desk/mod.rs` **kopiert, nicht verschoben** (U6b). Opcodes
  und `control[]`-Zaehler wie EmuTOS `desk/gembind.c`; `global[]` ein Feld je
  Anwendung (m68k-Breiten, `docs/api/aes/application.md` §AESGLOBAL);
  `WF_NAME` in `addrin[0]` (die native Form, die auch der Desktop benutzt,
  `window.rs` `wf_str_ptr`).
* **Allocator** (`heap.rs`): Hunk-Politik von mintlib wie NewTOS sie
  ausliefert (`sdk/libc/overlay/mintlib/malloc.c`, MEM05 Teil B): erster Hunk
  8 KiB, Verdopplung je `Malloc` bis 16 MiB, groessere Anforderung = eigener
  Block, Rundung auf 8 KiB (`DEF_PAGESIZE`), first fit ueber adressgeordnete
  Freiliste, Schnitt vom Ende, Verschmelzen beider Nachbarn, leerer Hunk
  zurueck an GEMDOS. **Eine bewusste Abweichung:** ein leerer Hunk wird als
  Reserve behalten (mintlib gibt sofort zurueck und treibt bei
  alloc/free-Wechsel den Plan zur Obergrenze); nicht ueber die TOS-API
  beobachtbar. Ausrichtung > 16 per Ueberallokation; Wiedereintritt (nur aus
  einem selbst installierten Signalhandler moeglich) → `alloc` null,
  `dealloc` leckt, statt Listen zu zerstoeren. GEMDOS-Blockadresse wird nicht
  als 16-ausgerichtet angenommen (EmuTOS garantiert nur gerade).
* **Panic:** `Cconws("*** Rust panic at <datei>:<zeile>: <meldung>")`,
  dann `Pterm(-1)` (`PANIC_EXIT` = GEMDOS `ERROR`; Wahl dieser Laufzeit,
  TOS kennt keinen Code dafuer); zweiter Panic im Handler → sofort `Pterm`.

## 3. Pruefungen

| Pruefung | erwartet | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1345/0/1 | **1345/0/1** |
| Release-Warnungen rpi4 / qemu-virt | 64/94 | **64/94** beide, 0 Fehler |
| Clippy rpi4 / qemu-virt | 76/125 | **76/125** beide |
| Driftprobe / unsafe | 31 / 24 | **31 / 24** |
| SDK-Hosttests | 20/0/0 | **20/0/0** |
| `programs/` Clippy (Standard + `panic-test`) | neu | **0** Warnungen; Build 0 Warnungen |
| Produktkern vs. U5a-Kern | nur Stempel | `cmp` **16 Byte** = zwei Git-Rev-Stempel |
| Reproduzierbarkeit `HELLOGEM.NAP` | — | zweiter Bau gleicher sha256 |

**Negativkontrolle:** ohne `-C relocation-model=pic` (Zielstandard `static`)
**scheitert** derselbe Link an 20× `R_AARCH64_ABS64 … recompile with -fPIC`
in `.rodata` (`checks/u6a-negative-static-relocation-model.txt`). Das Flag ist
Pflicht, und `-z text` faengt sein Fehlen beim Linken ab, nicht erst der
Lader. `-Z build-std` ist **nicht** noetig (fat LTO uebersetzt das genutzte
`core`/`alloc` mit PIC neu; `compiler_builtins` brachte keine
Textrelokation).

## 4. Binaereigenschaften (Rust gegen C)

| | `HELLOGEM.NAP` | `HELLO2.NCP` (C) | `QED.NAP` (C-GEM-App) |
|---|---|---|---|
| Typ / Einsprung | ET_DYN / 0x1000 | ET_DYN / 0x1000 | ET_DYN / 0x1000 |
| Programmkoepfe | LOAD R-X, LOAD RW-, DYNAMIC, NOTE | gleich | gleich |
| `DT_*`-Marken | = QED | ohne `RELA*` (keine Relokation) | Referenz |
| Relokationen | 28× RELATIVE | 0 | 569× RELATIVE |
| text / data / bss | 24 384 / 896 / 48 B | — | — |
| Dateigroesse | 100 240 B, davon ~75 KB DWARF aus dem vorkompilierten `core`/`compiler_builtins` (nicht geladen) | 5 968 B | — |

## 5. QEMU raspi4b (Einzelheiten: RUNINFO §4)

| Lauf | on | off | strict |
|---|---|---|---|
| `#Z 01 C:\HELLOGEM.NAP@` (Injektor-Kern, Return nach 8 s) | PASS | PASS | PASS |
| Desktop → F1 → HELLOGEM (`sh_nextapp=0`, Desktop-Startweg) | PASS | PASS | PASS |
| `#Z` HGPANIC (Injektor-Kern) | PASS¹ | PASS | PASS |
| `#Z` HGPANIC (**Produktkern**, unveraendert) | PASS | PASS | PASS |
| `#Z` HELLOGEM (Produktkern, ohne Taste) | Fenster gezeichnet, wartet wie vorgesehen | — | — |

Zeile des Programms (alle sechs HELLOGEM-Laeufe gleich):
`[HELLOGEM] ap_id=0 vwk=2 redraws=1 quit by key 0x1c0d; heap OK allocs=7726
Malloc=8 Mfree=7 peak_hunks=8 end: hunks=1 live=10; stack used 2464 of 65536
bytes; appl_exit=1`. Panic-Laeufe: `*** Rust panic at hello-gem/src/lib.rs:308:
deliberate panic (panic-test build) after redraw 1`, `[launch] returned
(program Pterm'd)`, Desktop oben.

¹ Erster Lauf unter Hostlast (checks.sh baute parallel): Programmteil korrekt
bei 15 s, dann QEMU 670 s ausgehungert (QMP-Antwort und naechster
Injektor-Tick erst bei 690 s) — bekannte Host-Stall-Klasse (MEM05 VOID).
Wiederholung im Leerlauf: PASS, identisch. Beide Laeufe liegen bei.

## 6. Befunde Stack / Allocator

* **Stack:** HELLOGEM braucht **2 464 von 65 536 Byte** (Bemalung des
  ungenutzten Stacks beim Start, Niedrigwassermarke am Ende; schliesst
  Heap-Test, `format!`, BTreeMap, Fenster- und Redraw-Pfad ein). Fuer den
  Desktop ist das **keine** Aussage (tiefe `form_do`/`dir`-Pfade); das
  Messverfahren liegt in `hello-gem` und kann fuer U9 in die Laufzeit wandern.
* **Allocator:** 7 726 Allokationen (3 000 `format!`-Strings, ein `Vec<u64>`
  auf 20 000 Elemente, 2 000 Boxen mit Luecken-Wiederbelegung, BTreeMap mit
  1 500 Eintraegen, je gegen unabhaengig berechnete Werte geprueft) aus **8**
  GEMDOS-Bloecken (Spitze 8 Hunks / 832 KiB; nach dem Abbau 1 Hunk = die
  Reserve, `live 0`). Kernel-Zeile nach dem Programm: `MDs in_use=3
  capacity=117 (+0 pages)` — der MD-Vorrat ist nie gewachsen; `in_use=3`
  hinterlaesst auch ein C-Programm (MEM05-Abnahme, HELLOMEM).

## 7. Offen / Fragen fuer U6b

1. **Exitcode nicht beobachtbar:** `launch.rs` `run_program_with_tail_rc`
   liefert `E_OK` fuer jedes Programm, das `Pterm` erreicht (dort so
   dokumentiert); die Shell-Zeile zeigt deshalb `rc=0` auch fuer `Pterm(-1)`.
   Der Wert −1 ist nur **statisch** belegt (Disassembly
   `rust_begin_unwind`: `Cconws`, dann `mov x0,#-1; mov w8,#0x4c; svc #1`).
   Ob `sh_ldapp` den Kindcode wie EmuTOS (`ret = dos_exec(…)`) braucht, ist
   eine Frage fuer U9 (Absturzpolitik D6), nicht fuer U6a.
2. **Doppelklick nicht gefahren** (kein Maus-Injektor fuer raspi4b); der
   Desktop-Startweg lief ueber F1 (`process_funkey` → `do_aopen_drop`),
   nicht ueber `hndl_button` → Doppelklick → `do_aopen`.
3. **Typ-Heimat:** `Grect` u. a. liegen jetzt doppelt (`newtos-gem::aes` und
   `desk/global.rs`). U5b (`tos-types`) sollte vor U6b entscheiden, wo GEM-
   Typen und `WF_*`/`WM_*`-Konstanten wohnen — sonst kopiert U6b die Drift.
4. **Panic-/Debug-Ausgabe** geht ueber `Cconws` auf den GEM-Bildschirm (und
   seriell); im Desktop-Fall kollidiert das mit D4 (Debug-Kanal).
5. **Dateigroesse:** ~75 % DWARF aus vorkompilierten Bibliotheken; `--strip-debug`
   im Link wuerde ~75 KB sparen, waere aber eine Abweichung von der
   `sdk/build.sh`-Zeile — Entscheidung fuer U6b/U9 (Blob im Kernel).
6. Review §7.4/2 (Pfad-Abhaengigkeit des Kernels in das ausgeschlossene
   `programs/`) ist mit U6a **nicht** geprueft; `newtos-gem` ist heute nur
   Mitglied des `programs/`-Workspace.
7. `programs/Cargo.lock` gehoert in den Commit (eigener Workspace).

## 8. Ausserhalb des Auftrags (nicht angefasst)

* `kernel/src/launch.rs:62-65,172`: Rueckgabe ist nie der Exitcode des Kindes (siehe 7.1).
* Die Root-`.gitignore` nennt `programs/*/target/`; der `programs/`-Workspace baut nach `target/programs/` (ignoriert ueber `/target`), die Zeile ist damit wirkungslos, aber unschaedlich.
* Der Kommentar im Root-`Cargo.toml:9-11` spricht von „own workspace + linker scripts" — nach T1 gibt es kein eigenes Linkerskript.

## 9. Selbstpruefung

Aufgabe in einem Satz: Rust-EL0-Spike `HELLOGEM.NAP` auf eigener
Laufzeit-Crate, gelinkt wie C, ohne Kernel-Aenderung, in drei memprot-Modi
gefahren. Geaendert/neu: nur `programs/` (12 Dateien + `Cargo.lock`), dieser
Bericht, das Messverzeichnis. Beide Boards bauen unveraendert (64/94,
76/125); der Produktkern ist bis auf den Git-Stempel byte-gleich. Scratch:
`/tmp/m5j/u6a` (Kernel-Kopie mit Injektor, Karten). Repo-Root
`kernel8.img` (ignoriert) von `checks.sh` ueberschrieben.
