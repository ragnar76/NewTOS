# ARCH03 Stufe 3b — Quittung

**Status: UMGESETZT bis auf einen offenen Punkt (E10-Verdrahtung, Entscheidung
nötig) · in QEMU auf beiden Boards in allen drei Modi belegt · Abnahme durch
den Tester offen · Pi offen** · 2026-10-05 · Zweig `arch03`, Basis HEAD
`31dd630`, Baum vorher sauber, nichts committet · Belege
[measurements/measurement-2026-10-05-arch03-3b/](../measurements/measurement-2026-10-05-arch03-3b/)
(`RUNINFO.TXT`, `checks/`, `serial/`, `screendumps/`, `method/`, `bin/`).

Jeder Prozess hat jetzt eigene Rechte auf den gemeinsamen Adressraum,
FreeMiNTs `PROT_P/G/S/PR/I` (`references/freemint/sys/arch/mprot030.c`).
Standard: an. Bootargument `memprot=off` = Stufe 3a unverändert,
`memprot=strict` = FreeMiNTs `MPF_STRICT`.

## Teilschritte

| | Stand |
|---|---|
| 1 Schutzmodus je Block (PRGFLAGS-Notiz für TPA/Env, `Mxalloc`-Nibble, `Malloc` = PRGFLAGS), Bootschalter | fertig |
| 2 Tabellen je Adressraum: geteilte L3 = Nicht-Besitzer-Rechte, eigene L3 wo Rechte abweichen; 4-KiB-Raster (E16); BBM/TLBI nach 0a | fertig |
| 3 Besitzwechsel (Mfree, Mshrink, Pexec 3/4/6, Ptermres, Pterm), E5 + strict, E10, AES-kalias | fertig **außer E10-Verdrahtung** (Mechanismus gebaut + getestet, nicht angeschlossen — siehe Offen 1) |
| 4 Schutzverletzung → EmuTOS-Absturzbericht + Zeile „Memory violation: …“ (Übersetzungs- vs. Rechtefehler, Regionstabelle entscheidet) | fertig |
| Nebenbefunde 3a: `shel_envrn`, `vr_trnfm`, `Fsetdta` aus EL1 | fertig |

**Offen gelegt:** Teilschritte 1–4 sind gemeinsam implementiert und an
Prüfpunkten c1–c5 gemeinsam geprüft, nicht je Teilschritt einzeln (wie 3a);
die OFF-Serie ist der Prüfpunkt für Teilschritt 1, ON/STRICT für 2–4.

## Entwurfsentscheidungen (meine, offen gelegt)

- **Rechte werden berechnet, nicht fortgeschrieben.** Jede Änderung
  (Allokation, Freigabe, Besitz, Modus, Prozessstart/-ende) markiert einen
  Adressbereich bzw. einen Slot als schmutzig; `memprot::sync()` rechnet die
  betroffenen 2-MiB-Regionen aus Pool-Listen, Besitzern (`m_own` + weitere
  Besitzer) und Blocksätzen neu. `sync` läuft am Ende jeder mutierenden
  BDOS-Operation, vor jedem `install_ttbr0` und am Ende jedes Traps
  (Schnellpfad ohne Barriere, wenn nichts ansteht).
- **Besitzer = Speicherraum:** ein Slot hat Besitzerrechte, wenn ein Besitzer
  des Blocks ein Prozess seines Raums ist (er selbst oder — `Pexec(4)` — der
  Raum des Aufrufers, E6), oder der Block bei seinem Start dem Raum des
  Elternteils gehörte (E5, nicht strikt), oder eine E10-Freigabe besteht.
- **Tabellen:** jeder Slot hat eigene L2 für alle vier GiB (Kopien der
  geteilten Vorlagen), damit eine private L3 auf beiden Boards eine Änderung
  eines slot-eigenen L2-Eintrags ist (V0a-Invariante 4: virt-Pool liegt in
  GiB 1). Pool-L3 (geteilt) und private L3 kommen aus einem Tabellen-Vorrat,
  den der Kernel beim Booten vor den Pools aus den Frames nimmt (Pi: 1520
  Tabellen ≈ 5,9 MiB, virt 1172), nur bei Schutz an. L2-Zeigerwechsel per
  BBM (ungültig → `dsb ish` → `tlbi vmalle1is` → `dsb ish` → neu → `dsb`),
  Blattänderung eines gültigen Eintrags → `tlbi vmalle1is` vor dem `eret`
  (Pi bestätigt: keine QUIRK-001-Klammer nötig). Keine ASIDs (Stufe 4); jeder
  Wurzelwechsel invalidiert weiter den ganzen TLB.
- **Kernel-Blöcke `PROT_S`:** ein Kernel-Aufrufer ohne Nibble bekommt
  `PROT_S` (FreeMiNT `sys/memory.c:150-159`). Ein Pool-Block, den der Kernel
  per `usermem::expose` veröffentlicht (`WF_SCREEN`), nimmt den Modus der
  Veröffentlichung an.
- **Absturzzeile:** FreeMiNTs Klassifikation (`report_buserr`) als **eine**
  Textzeile im EmuTOS-Bericht (E11: kein Alert), nur bei Schutz an.
- **Default-Modus fehlender Satz:** `PROT_P` (kann nur durch einen Fehler
  entstehen; jeder Block bekommt bei Schutz an einen Satz).

## Was getan wurde

| Bereich | Datei / Funktion |
|---|---|
| Rechte-Maschine | neu `tos-api/src/bdos/process/memprot.rs` (`configure`, `pool_align_mask`, Blocksätze `record/mode_of/rekey`, `on_alloc/on_freed/set_mode/on_owner_change/on_resident`, `touch/sync/refresh`, `on_root_built/on_launch/on_slot_released`, `grant_temp/revoke_temp`, `block_containing`, `region_kind`) |
| Tabellen | `tos-api/src/bdos/process/mmu.rs`: `SlotTables.l2[4]`, Tabellen-Vorrat (`set_table_arena`, `arena_alloc/free`), `init_slot_tables` gibt private Tabellen frei, `set_l2` schont private Einträge, `shared_l3_for` aus Vorrat, `map_pools` (an: nur 4-KiB-Seiten, alles ungültig), `map_shared` gibt Veröffentlichungen an private Kopien weiter, `shared_l3/slot_private_l3/make_private/drop_private/replace_bbm/set_leaf/flush_tlb(_always)`, `install_ttbr0` → `sync` |
| BDOS | `bdos/mem/traps.rs` (`mxalloc` Nibble + Satz, `mxalloc_aligned` rekey, `set_owner`, `mfree`, `mshrink` → Rechte), `pool.rs` `PoolConfig::current()` (E16), `region.rs` (Besitzwechsel → Rechte, `further_owners`, `Ptermres` P→G) |
| Prozesse | `process.rs` `free_slot` → `on_slot_released`; `loader.rs` TPA/Env-Modus aus PRGFLAGS, `install_slot` → `on_root_built`; `pexec.rs` `set_parent` → `on_launch`; `usermem.rs` `expose` im Pool → Modus |
| Absturz | `bios/crash.rs` `RegionKind/FaultType/Access/MemViolation/mem_violation` + Berichtszeile; `kernel/src/arch/aarch64/exceptions.rs` (beide Berichtspfade; `rust_sync_handler` → `sync` am Trap-Ende) |
| Boot | `kernel/src/bootargs.rs` (`memprot=off`, `memprot=strict`, jetzt für alle Boards), `kernel/src/arch/aarch64/dtb.rs` `copy_bootargs` (virt: `/chosen/bootargs`), `kernel/src/lib.rs` `memprot_switch`, Vorrat in `bring_up_bdos_pools` |
| Nebenbefunde | `aes/shell.rs` `shel_envrn` (eigene Seite `AES_ENV`, für EL0 schreibgeschützt veröffentlicht), `vdi/raster.rs` `vr_trnfm` (vorzeichenbehaftet wie EmuTOS, `fd_stand` immer gesetzt), `bdos/fs/find.rs` `fsetdta/fgetdta/fsfirst/fsnext` + `gemdos/trace.rs` (DTA nach Aufrufer-Space) |
| Selbsttest | `kernel/src/selftest_d7.rs`: erwartete Restlänge = `blkln` auf das Pool-Raster gerundet |
| Doku | `docs/api/gemdos/memory/{44-mxalloc,48-malloc,49-mfree,4a-mshrink}.md`, `docs/api/gemdos/process/{4b-pexec,31-ptermres}.md`, `docs/api/gemdos/dta/1a-fsetdta.md`, `docs/api/aes/scrap-shell.md`, `docs/api/bios/_exception-crash.md` (Abschnitt „Memory protection“), `docs/COMPATIBILITY.md` (sanktionierte Abweichung inkl. E16), `tos-types/src/memory.rs` Kommentar, [MEM04](MEM04.md) geschlossen |
| Tests | `tos-api/tests/arch03_3b_memprot.rs` (13), `arch03_3b_fsetdta.rs` (1), `vdi_raster_reference/harness.rs` (1), `bios::crash` (2), `bootargs` (1) |
| Sonden | `sdk/examples/mprot3b.c` + `mpkid.c` (3 Erwartungsvarianten), `measurements/…/method/tests/envrn.c` |

## Prüfungen (Zahlen, Einzelheiten `RUNINFO.TXT`)

| | 3a | c5 |
|---|---|---|
| Hosttests | 1104/0/1 | **1123/0/1** |
| Release frisch, tos-api/kernel, beide Boards | 64/94 | **64/94** |
| Clippy tos-api/kernel, beide | 76/125 | **76/125** (c1: 81 — 5 neue, behoben) |
| Driftprobe | 31 | **31** |
| SDK-Hosttests | 13/0/0 | **13/0/0** |

QEMU c5, **ON und OFF** je: Desktop, TeraDesk, QED, APITEST-Bilder
**bytegleich** mit 3a; APITEST 106/0/5; Doom VERIFIED (Einzelbilder
zeitabhängig, in beiden Modi gleich unstet); PORT10 42/44 (PROC02 wie
vorher); CRASHTST, UACC3A (26 Fälle), KBDVEC + EmuCON (bytegleich),
Accessory-Start, virt-Desktop, virt-Selbsttests wie 3a (sleepwake dieselben
5 FAIL), desk3a (Unterschiede nur Dateizeit/Dateizahl der Karte), ENVRN PASS.
**MPROT3B PASS** in ON, OFF und STRICT auf `raspi4b` und `virt` (Tabelle in
`RUNINFO.TXT`). **S2ALL:** OFF PASS; **ON FAIL in genau einer Prüfung von
TSRPAR** (siehe Offen 2) — alle funktionalen TSR-Prüfungen bestehen.

## Verhalten bei Schutzverletzung

Beispiel `memprot=strict`, Kind schreibt privaten Block des Elternteils:
`Panic: Bus Error` · `esr=92000047` · `addr=…` · **`Memory violation: private
memory, write (translation fault, level 3)`** · Register · Taste ·
`Pterm(-1)`, Eltern-`Pexec` = `0x0000FFFF`, Eltern unversehrt. `PROT_PR`
schreiben → `readable memory, write (permission fault, level 3)`; Schreiben
nach `Mfree` → `free memory, write (translation fault, level 3)`; schlechter
Zeiger außerhalb der Pools → `hardware memory, …`.

## Für den Tester

- Karten: `method/seed-card.sh` (nie `tools/make-sd-image.sh`), Serie
  `method/runs-3b.sh <label> <cards> on|off|strict`, Modus über
  `-append` (`APPEND`-Variable der Laufskripte), Sonde `C:\MPROT3B.NAP`
  (an) / `MPROTS.NAP` (strict) / `MPROTO.NAP` (aus) + `C:\MPKID.NAP`,
  Crash-Kernel mit Injektor `bin/kernel8-crash-c5.img`, virt
  `bin/kernel-virt-crash-c5.img` (wählt die Variante selbst).
- Erwartung: „MPROT3B: PASS“ in jedem Modus; ON: ein Absturz (Fall f);
  STRICT: vier (p, P, R, f); OFF: keiner.
- Pi: `bin/kernel8-c5.img`; `cmdline.txt` mit `memprot=off` bzw.
  `memprot=strict` schaltet um. Pi-Lauf nur auf Ansage.

## Offen / zur Entscheidung

> **Stand nach 3c (2026-10-05):** 1 (E10), 3 (E6 vs. strict), 4 (`F_KEEP`)
> und 5 (Tabellenmangel) sind in Stufe 3c geschlossen. **2 ist nicht
> geschlossen:** die Lader-Reihenfolge ist jetzt die von EmuTOS, die
> TSRPAR-Prüfung scheitert unter Schutz an/strict aber weiter — die Ursache
> ist nicht die Reihenfolge, sondern E9 + E16 (Messung und Begründung:
> [ARCH03-3c-STATUS-UNFINISHED](ARCH03-3c-STATUS-UNFINISHED.MD)). Die Datei
> bleibt deshalb „UNFINISHED“.

1. **E10 nicht verdrahtet.** `memprot::grant_temp/revoke_temp` sind gebaut
   und getestet (Hosttest `temporary_rights_for_a_foreign_callout`). Sie
   anzuschließen heißt, in `aes/object.rs` `ob_user` die A97-Regel
   aufzuheben („Callout nur, wenn der Baumeigentümer der laufende Prozess
   ist“) und fremde `G_USERDEF`-Routinen im laufenden Prozess mit
   zeitweiser Freigabe auszuführen (EmuTOS zeichnet sie; Plan Frage 9 E10a).
   Das ändert das Zeichenverhalten **auch mit `memprot=off`** — Widerspruch
   zur Vorgabe „Schutz aus = 3a exakt“. Ich habe deshalb nicht verdrahtet.
   Entscheidung des Leads: (a) A97 aufheben in beiden Modi, (b) nur bei
   Schutz an, (c) so lassen.
2. **S2ALL/TSRPAR „TPA beyond blkln back in the pool“ unter ON.** Die
   Prüfung misst den größten freien Block; der TPA-Rest geht zurück (D7
   belegt es über die TT-Belegung), liegt aber nicht mehr am großen freien
   Block an. Ursache: der Lader holt die TPA **vor** der Umgebung, EmuTOS
   die Umgebung zuerst (`bdos/proc.c:281-292`); mit 16-Byte-Raster fielen
   Umgebung und kleine Blöcke in den Ausrichtungsrest unter der TPA, mit
   Seitenraster (E16) gibt es den nicht. Korrektur (Reihenfolge wie EmuTOS)
   liegt außerhalb dieses Auftrags und verschiebt auch das OFF-Layout.
3. Abweichung Auftrag ↔ E6: der Auftrag nennt „Pexec 4 child reads parent
   memory (E5 default) and fails in strict mode“. Nach E6 (Anwenderentscheid
   2026-10-05) läuft ein `Pexec(4)`-Kind im Raum des Aufrufers und sieht ihn
   in jedem Modus (FreeMiNT `FORK_SHAREVM`). Gebaut und geprüft: `Pexec 4`
   sieht in allen Modi; das E5/strict-Verhalten an `Pexec 0`-Kindern.
4. `F_KEEP` (`0x4000`) im `Mxalloc`-Modus wird weiter nicht ausgewertet.
5. Tabellen-Vorrat erschöpft → der Prozess behält dort die Nicht-Besitzer-
   Rechte (verliert Zugriff, gewinnt nie welchen); Zähler
   `memprot::table_shortfalls()`, noch nicht im Boot-/Statusprotokoll.

## Beobachtungen außerhalb des Auftrags (unverändert)

- Lader-Reihenfolge TPA vor Umgebung (Offen 2).
- `selftest_sleepwake_switch` 5 FAIL vorbestehend.
- Doom-Einzelbilder bei t30/t45 laufen je Lauf unterschiedlich (zeitabhängige
  Demo), unabhängig vom Modus.
