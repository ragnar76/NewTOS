# ARCH03 Stufe 1 — Quittung

**Status: IMPLEMENTIERT · in QEMU auf beiden Boards belegt · Abnahme durch den
Tester offen · Pi offen · zwei Punkte zur Entscheidung (unten)** · 2026-10-05 ·
Zweig `arch03`, Basis HEAD `c806a64`, Baum vorher sauber, nichts committet ·
Belege [measurements/measurement-2026-10-05-arch03-1/](../measurements/measurement-2026-10-05-arch03-1/)
(`RUNINFO.TXT`; `checks/` statische Pruefungen je Teilschritt, `serial/` +
`screendumps/` QEMU-Laeufe, `method/` Skripte und Diffs, `bin/` Kerne und
Programme).

Ein gemeinsamer Anwenderadressraum ohne Schutz (FreeMiNT mit `no_mem_prot`,
Plan §3.1/§3.2, E1/E3/E9/E18 wie empfohlen): jede Anwenderadresse ist ihre
physische Adresse, nur Seite 0 ist je Prozess. Programme laufen an ihrer
Pool-Adresse, die Basepage ist der Anfang des TPA-Blocks und damit der echte
PD-Zeiger; `Pexec 3` liefert ihn, `Pexec 4` startet ihn; `p_parent` wie EmuTOS;
die Umgebung ist fuer jeden lesbar (MEM03).

## Teilschritt 1a — Lader: Positivliste der dynamischen Marken

Lead-Entscheidung zum 0d-Befund `DT_RELR`: unbekannte Marken werden abgelehnt.

- `tos-api/src/bdos/process/reloc.rs`: Positivliste — gABI-Marken (ch5.dynamic,
  gepinnt) `DT_HASH/STRTAB/SYMTAB/STRSZ/SYMENT/DEBUG/INIT_ARRAY/FINI_ARRAY/
  INIT_ARRAYSZ/FINI_ARRAYSZ`, `DT_PLTRELSZ`/`DT_RELSZ` = 0, `DT_FLAGS` ohne
  `DF_TEXTREL`, dazu die drei Marken, die rust-lld in jedes Programm schreibt
  (`GNU_HASH` 0x6ffffef5, `RELACOUNT` 0x6ffffff9, `FLAGS_1` 0x6ffffffb;
  gemessen mit `llvm-readobj -d` an QED, TeraDesk, Doom, APITEST, HELLO2 — in
  keiner gepinnten Quelle, im gABI-Bereich zwischen `DT_HIOS` und `DT_LOPROC`).
  Jede andere Marke → `RelocError::UnknownTag(tag)` → `EPLFMT`.
- `tos-api/tests/loader_pie_reloc.rs`: +2 Tests.

Pruefungen `checks/s1a-*`: Hosttests 1047/0/1, Release 64/94 beide Boards,
Clippy 80/125, Driftprobe 31, SDK 13/0/0; QEMU (`serial/s1a-*`) Desktop,
TeraDesk, QED, Doom VERIFIED, APITEST 106/0/5, virt Desktop.

## Teilschritt 1b — der gemeinsame Adressraum

| Datei | was |
|---|---|
| `tos-api/src/bdos/process/mmu.rs` | neu geschrieben: Wurzel je Slot (L0, L1, L2 fuer GiB 0, L3 fuer [0, 2 MiB)) ueber **geteilten** Tabellen (L2 fuer GiB 1–3, Vorlagen fuer GiB 0 und [0, 2 MiB), 64 L3-Tabellen fuer Teilbereiche); `page0_kva`/`page0_bytes` (Seite 0 je Slot, eigene Kernelseite); `map_pools` (beide Pools beim Booten, 2-MiB-Bloecke wo ganz, sonst Seiten), `map_shared` (nur ungueltig→gueltig, abweichende Attribute = `Conflict`, Seite 0 nie), `init_slot_tables`, `translate` (Software-Walk fuer Tests). Weg: `USER_L3_COUNT`, 1216 KiB Tabellen-BSS |
| `tos-api/src/bdos/process/usermem.rs` | Fenster zurueckgebaut (Plan Frage 5): weg sind `map`, `seed_tpa`, `translate`, `free`, `free_by_mstart`, `window_remaining`, Blocktabelle, Bump-Cursor. Bleibt: `expose`/`expose_attr` (Kernelseite bei VA = PA in die geteilten Tabellen, Pool-Speicher wird nur zurueckgegeben), `MemAttr::Uncached` + `UNCACHED_SLOTS` (Clean+Invalidate beim ersten lebenden Halter), `resolve_kva` (Seite 0 des Slots, belegter Poolblock oder Veroeffentlichung → Kernel-Alias; freigegebener Speicher → `None`) |
| `tos-api/src/bdos/mem/{pool,traps}.rs` | `pool::trim_front` + `traps::mxalloc_aligned` (seitenausgerichteter TPA-Block, Rest zurueck in den Pool); `traps::is_allocated` |
| `tos-api/src/bdos/process/arena.rs` | TPA = seitenausgerichteter Poolblock (4 MiB, E9), Adresse = PD; `page_bytes_mut` = Seite 0, neu `basepage_bytes_mut` |
| `tos-api/src/bdos/process/loader.rs` | Ladebasis = Blockadresse B (Relokation mit B, Eintritt B + `e_entry`, SP = B + 4 MiB); Basepage bei B mit B-relativen Feldern; Seite 0 getrennt (Sysvars, Jar, Trampoline, KEYS02); **`os_run`-Zelle `0x5B8`** (8 Byte, eigener PD); Besitz TPA **und** Umgebung nach EmuTOS (`owner = PE_LOADGO ? p : run`); `p_env` = Pooladresse (MEM03); `Inherit` ueber den Kernel-Alias; Code-Cache-Pflege fuer Bild und Seite 0 |
| `tos-api/src/bdos/process.rs` | `RUN` = **PD-Zeiger** (Anwenderadresse); `current_kva()` zum Lesen, `current_page0_kva()`, `pd_of(slot)`, `kalias`/`kva_to_user_va` (Host: Identitaet); `slot_for` ueber den Alias; `m_own` = PD (Plan Frage 13); Feld `load_owner` |
| `tos-api/src/bdos/process/pexec.rs` | `PE_LOAD` gibt den PD zurueck; `PE_GO` nur fuer einen ungestarteten `PE_LOAD`-PD (sonst `EIMBA`); `set_parent` (EmuTOS `proc_go`); `release_unstarted_of` (ungestartete `PE_LOAD`-Slots gehen mit ihrem Lader); Parameter `owned_by_child` |
| `tos-api/src/bdos/process/pterm.rs` | Eltern-`RUN` = PD; ungestartete Kinder freigeben |
| `tos-api/src/gemdos/memory.rs` | `Malloc`/`Mxalloc`/`Mfree`/`Mshrink` direkt auf die EmuTOS-Traps, `Malloc(-1)` = groesster freier Block; Freigaben an `aes::resource::on_mfree` |
| `tos-api/src/aes/resource.rs` | `.RSC`-Blob und Farbicon-Puffer als Poolbloecke an ihrer Adresse; `on_mfree` markiert **jeden** Eintrag mit dem Schluessel (im gemeinsamen Raum kann ein Prozess fremde Bloecke freigeben, E15 erst Stufe 2) |
| `tos-api/src/aes/object.rs` | `ObjTree::new` gibt dem Baum eines EL0-Aufrufers seinen Besitzer → jeder Lesezugriff ueber `resolve_kva` (Plan §3.6 „AES liest App-Baeume per kalias"). Ausloeser: APITEST T31 (`menu_bar` eines 2-Objekt-Baums) las hinter dem Feld in den unabgebildeten Bereich unter dem TT-Pool → Kernel-Data-Abort/Halt (`serial/wip1/wip2-*`); jetzt NIL-Satz |
| `tos-api/src/xbios/{keytbl,kbdvecs}.rs`, `bios/{chardev,tick}.rs`, `aes/object.rs` (PARMBLK), `kernel/.../exceptions.rs` (Userdef-Rueckgabe) | Seite-0-Zugriffe ueber `mmu::page0_kva` statt `basepage_kva` |
| `tos-api/src/bdos/fs/{find,file}.rs`, `kernel/.../crash.rs` | Basepage ueber `current_kva()` |
| `kernel/src/lib.rs` | `mmu::map_pools()` nach dem Seeden der Pools |
| `kernel/src/arch/aarch64/exceptions.rs` | `apply_switch`: Erststart mit `x0` = PD (0d-Punkt); `handle_pexec`: `p_parent`, `RUN` = PD |
| `kernel/src/{launch,accs}.rs` | Kind besitzt TPA; `RUN` = PD |
| 9 Selbsttests | PD statt Kernel-VA, Handler-/ELR-Adressen relativ zur Blockadresse, Merker in Seite 0 |
| SDK | `apitest.c` (Basepage ueber `_base`, `el0_readable`-Grenze 2 GiB), `examples/cmdline.c`/`crchild.c` (Basepage ueber `*(_sysbase->os_run)`), `wfscreen.c` (Grenze 2 GiB); Kommentare `newtos.ld`, `crt0.S` ×2, `heap_init.c` |
| Hosttests | neu `tos-api/tests/arch03_shared_space.rs` (7), +2 Lader-Unittests; angepasst `xbios_kbdvecs.rs`, Jar-Spiegel-, Ressourcen-Tests; weg 6 Fenster-Sondentests in `gemdos/memory.rs` |

## Teilschritt 1c — Dokumentation

`docs/api/{structures/basepage,gemdos/memory/44-mxalloc,48-malloc,49-mfree,4a-mshrink,gemdos/process/4b-pexec,bios/00-getmpb,07-getbpb,xbios/02-physbase,03-logbase,10-keytbl,aes/resource-fileselector}.md`,
`docs/COMPATIBILITY.md` (neuer Eintrag „Ein gemeinsamer Adressraum, Seite 0 je
Prozess, TPA fester Groesse" + zwei Korrekturen), `docs/ARCHITECTURE.md`,
`docs/sdk/{05-abi-pitfalls,07-mintlib-limits}.md`, Code-Kommentare.
`docs/STATUS.md` und `docs/MEMORY.md` nicht angefasst (ADR-025 bleibt im Plan
bis zur Freigabe).

## Pruefungen (final, `checks/s1-*`)

| | Ergebnis | Basis |
|---|---|---|
| Hosttests | **1050 / 0 / 1** | 1045 / 0 / 1 (+2 1a, −6 Fenster-Sonden, +7 neu, +2 Lader) |
| Release frisch je Board | `tos-api` **64**, `kernel` **94** | 64 / 94 |
| Clippy beide | **80 / 125**, Warnungsverteilung je Datei identisch zu 1a | 80 / 125 |
| Driftprobe | **31** | 31 |
| `sdk/run-host-tests.sh` | 13 / 0 / 0 | gruen |

QEMU (alle aus neu gebauten Binaerdateien; Einzelheiten `RUNINFO.TXT`):
Desktop, TeraDesk, QED (Vollbild), Doom (`VERIFIED via EdDI+scale+vro_cpyfm`),
APITEST **106 / 0 / 5** (wie 0d), CRASHTST **PASS** (raspi4b und virt),
KBDVEC **PASS** + EmuCON (`run hello2.ncp`, `run oldexec.ncp` → Fehler 66),
**MEM03: PASS** (Kind liest `p_env` Satz fuer Satz), **P34: PASS** (Pexec 3 →
Eltern lesen die Kind-Basepage → Mshrink → Pexec 4 → `p_parent`, `os_run`,
Besitz), WFSCREEN PASS, PHYSBASE (Bänder bei `0x3C100000`), Accessory
(Erststart durch den Scheduler), PORT10 42/44 (die 2 Fehler wie in PORT10d,
vorbestehend), Selbsttests 9 PASS + `sleepwake_switch` mit denselben 5
Fehlern wie vor 0d, virt Desktop.

## Offen / zur Entscheidung

1. **E6 woertlich vs. E3.** Plan Frage 7: das `Pexec 4`-Kind „erhaelt dieselbe
   TTBR0-Wurzel wie der Aufrufer". Mit Seite 0 je Prozess (E3) saehe das Kind
   dann die Seite 0 des Aufrufers — `*(os_run)` naennte den Aufrufer, die
   KEYS02-Tabelle waere geteilt. Umgesetzt: das Kind behaelt seine eigene
   Wurzel; in Stufe 1 teilen alle Wurzeln alles ausser Seite 0, also ist der
   Speicher des Aufrufers der des Kindes. Fuer Stufe 3 heisst das: Kind-Wurzel
   = Rechte-Tabellen des Aufrufers + eigene Seite 0. Bitte bestaetigen.
2. **MEM04.** Die Fensterabbildung und damit die prozesseigene veraltete
   Abbildung gibt es nicht mehr. Freigegebener Speicher bleibt aber fuer alle
   abgebildet (wie EmuTOS) — dass ein Prozess nicht in den Block eines anderen
   schreiben kann, kommt erst mit Stufe 3 (I8).
3. **0c-Liste uaccess.** Umgestellt sind die vom Plan fuer Stufe 1 genannten
   AES-Lesewege (Baeume ueber `resolve_kva`, Ressource ueber den Alias).
   Die GEMDOS/BIOS-Direktzugriffe der 0c-Liste (`Cconrs`, `Ssystem`,
   `Setexc`, `conterm`, `bdos/fs` Pfade/Puffer/DTA, `Getmpb`, `Protobt`) und
   die AES-Parameterbloecke (`aes/mod.rs`) lesen weiter privilegiert ueber
   `TTBR0` — funktioniert, weil VA = PA, faellt aber bei einem schlechten
   Zeiger als Kernel-Abbruch. Laut Plan §3.6 Auditor-Inventur vor Stufe 3.
4. **Farbicon-Abweichung** (`aes/resource.rs`, ein Puffer je Ressource statt
   je Icon): ihr Grund (32-Eintraege-Fenstertabelle) ist weg; Kandidat zum
   Zurueckbauen, in Stufe 1 nicht angefasst.

## Fuer die Pi-Karte

Kernel und alle geaenderten Programme muessen zusammen auf die Karte (ein
0d-Programm, das die Basepage bei VA 0 liest, laeuft nicht mehr):
`kernel8.img` (`cargo xtask build --board rpi4 --release`), `APITEST.NCP`,
`CMDLINE.NCP` + `CMDLINE.NTP`, `CRCHILD.NAP`, `WFSCREEN.NAP`; optional die
Sonden `ENVPAR.NAP` + `ENVKID.NAP`, `P34PAR.NAP` + `P34KID.NAP` (nach C:\).
QED, TeraDesk und Doom wurden neu gebaut und sind byte-gleich mit 0d; die
uebrigen 0d-Programme (CoreMark, libc/gemlib/cflib/libm-Tests, tosdump,
TESTGEM, CHAINTST) sind aus unveraenderten Quellen und lesen die Basepage
nicht bei VA 0 (Quelltextsuche) — in Stufe 1 nicht gefahren.

## Beobachtungen ausserhalb des Auftrags (unveraendert)

- `kernel/src/accs.rs` `free_accs`: der Kommentar sagt, `release_slot` gebe den
  TPA zurueck — es gibt nur den Slot frei, der TPA-Poolblock bleibt (vorbestehend).
- `selftest_sleepwake_switch` scheitert vor und nach 0d/1 mit denselben 5 Pruefungen.
- PORT10: die 2 Umleitungsfehler (`> file`) bestehen seit PORT10d (Handle-Vererbung, PROC02).
