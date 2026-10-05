# ARCH03 Stufe 0b — Spike: was die Quellen nicht entscheiden, gemessen

Stand 2026-10-05 · Auftrag: `TASK/ARCH03.md`, Plan `docs/analysis/arch03-implementation-plan.md` Stufe 0b, offene Punkte aus `docs/analysis/arch03-stage0a-armarm-verification.md` §11 (im Folgenden **V0a**). **Wegwerfkopie, kein Produktcode:** gemessen wurde in einer Kopie des Baums (`git archive d96fdf6`), nichts davon ist eingecheckt. Messprotokoll: `measurements/measurement-2026-10-05-arch03-0b/` (`method/` = Diff + Startskripte, `serial/` = Logs, `shots/` = QMP-Bildschirmfoto).

Kennzeichnung: **[QEMU gemessen]** = auf QEMU 11.0.2 (`qemu-system-aarch64 --version`) gemessen, Log-Zeile angegeben · **[Pi offen]** = Messung auf der echten Pi 4 steht aus (Abbild und Anleitung unten, Ergebnistabelle §7) · **[Schluss]** = gefolgert.

---

## 0. Kurzfassung

| Frage | QEMU 11.0.2 `raspi4b` | QEMU 11.0.2 `virt` | Pi 4 |
|---|---|---|---|
| Q1 ID-Register | Cortex-A72 r0p3 (`MIDR 0x410FD083`); `ASIDBits = 2` (16 Bit); PAN/UAO/HPDS/HAFDBS/BBM = 0; `CTR_EL0 0x8444C004` (IDC = DIC = 0); `CLIDR 0x0A200023`; L2 1 MiB 16-fach | identisch | **[Pi offen]** |
| Q2 VA = PA neben dem TTBR1-Alias („Wand 3“) | **keine Wand**: 6/6 Varianten (Pool 2 MiB / 4 KiB, ganzes DRAM; je mit und ohne TLBI) und 4/4 Boot-Nachstellungen von ADR-010 laufen, Kernel- und EL0-Sicht kohärent | identisch | **[Pi offen]** |
| Q3 Wurzelwechsel | Klammer, Architekturfolge ohne Klammer, TLBI direkt vor `eret`, ASID-getaggt ohne TLBI: je 32/32 richtig; **ohne TLBI und ohne ASID: 16/32 falsch** (veralteter TLB-Eintrag, keine Hänger) | identisch | **[Pi offen]** |
| Q4 `ldtr`/`sttr` an EL1 | wie V0a vorhergesagt: Anwenderseiten ok; RO-Schreiben, EL1-only, ungültig L0–L3, Kerneladresse → `EC 0x25`, DFSC je Fall, `FAR` = VA, `WnR` richtig, `ELR` = PC des Befehls | identisch | **[Pi offen]** |
| Q5 Bildschirm-PA | `0x3C10_0000..0x3C35_8000` (VideoCore-Speicher oberhalb `dram.end`, < 1 GiB) | `0x5081_7000..0x5100_0000` (OS-Reserve im DRAM) | **[Pi offen]** |

**Wichtigste Folge:** Alle drei ADR-010/QUIRK-001-„Wände“ (UART-Klammer, ASID-`msr`, Kernel/Anwender-Aliasing) und die Regel „kein TLBI direkt vor `eret`“ lassen sich auf QEMU 11.0.2 **nicht mehr** auslösen — weder auf `raspi4b` noch auf `virt`, je dreimal reproduziert. Die Boot-Abbildung der Pools (Plan Frage 2) ist auf QEMU tragfähig; die Rückfallebene E18 („Abbildung bei Allokation“) wird auf QEMU nicht gebraucht. Ob die Pi dasselbe zeigt, entscheidet der Pi-Lauf (§6/§7) — die Architektur (V0a) erwartet es.

---

## 1. Methode

**Baum:** `git archive d96fdf6` in die Sitzungs-Scratchpad-Kopie; Änderungen nur dort (Diff: `method/arch03-0b-spike.diff`, 4 Dateien):

- `kernel/Cargo.toml`: Feature `arch03-spike`.
- `kernel/src/lib.rs`: `spike_arch03::early(&map)` direkt nach `mmu::reinstall` (wirkt nur mit Boot-Option `spike.early*`), `spike_arch03::run(&map)` an der Stelle von `initinfo::diag::show` (nach Begrüßungsbildschirm, vor AUTO/Desktop; endet in `halt`).
- `kernel/src/arch/aarch64/exceptions.rs`: Haken `spike_arch03::on_sync(frame)` als erste Anweisung von `rust_sync_handler`.
- `kernel/src/spike_arch03.rs` (neu): die Messung.

**Ablauf des Spikes** (jede Zeile geht auf UART **und** Framebuffer-Konsole; auf `board-rpi4` blättert die Ausgabe seitenweise mit Tastendruck, außer bei Boot-Option `spike.nopause`):

1. **Q1** `mrs` von `MIDR`, `MPIDR`, `ID_AA64MMFR0/1/2_EL1` (MMFR2 als `s3_0_c0_c7_2`), `ID_AA64PFR0`, `ID_AA64ISAR0`, `CTR_EL0`, `CLIDR_EL1`, `CCSIDR_EL1` je Ebene über `CSSELR_EL1`, `SCTLR_EL1`, `TCR_EL1`; Felder dekodiert.
2. **Q5** Framebuffer-Basis (`Framebuffer::mem_base` − `VA_OFFSET`), Größe, `dram`, Pool-Grenzen.
3. **Aufbau:** 4 MiB Tabellen-Arena und 64-KiB-Testblock per `Mxalloc(…, 3)` aus dem BDOS-Pool (VA = PA). In den Testblock: drei EL0-Routinen (`global_asm`, kopiert, `dc cvau` + `ic iallu`), zwei „private Seiten 0“ A/B mit Datenwort `0xA0A0A0A0`/`0xB0B0B0B0` bei Offset 0 und Code bei `0x100` (`movz x0,#0xA/#0xB; ret`) und `0x180` (Direkteinstieg: Tag, `ldr x1,[0]`, `svc`).
4. **EL0-Ausflug:** eigener `eret`-Rahmen nach dem Muster von `user_entry::enter_el0_with_resume_capture` (SPSR = EL0t mit DAIF maskiert); `svc #0xA03` oder jeder EL0-Abbruch landet im Haken, der `ESR/FAR/ELR/x0..x3` festhält und an EL1 zurückleitet.
5. **Q2** sechs Varianten `a2m-tlbi`, `a4k-tlbi`, `a2m-notlbi`, `a4k-notlbi`, `adram-tlbi`, `adram-notlbi`: Wurzel bauen (Pools VA = PA in 2-MiB-Blöcken mit 4-KiB-Rändern / alles 4 KiB / ganzes DRAM wie ADR-010), Einträge `AP=01, PXN=1, UXN=0, AF, SH=Inner, AttrIdx 0`; einbauen mit `dsb; msr; isb` bzw. zusätzlich `tlbi vmalle1; dsb ish`; **unmittelbar danach** ohne Gerätezugriff Normal-Speicher-Stores (Heap-`Box`, Testblock über den Kernel-Alias, `static`), EL1-Lesen über die niedrige VA, zwei EL0-Lese-/Schreibrunden auf derselben Seite, Rücklesen über den Alias. Bei den TLBI-Varianten zusätzlich ein Lastlauf (64 MiB, ein Load je 4-KiB-Seite, ×8).
6. **Frühe Nachstellung von ADR-010** (nur mit Boot-Option): `spike.early`/`spike.early-notlbi` bauen direkt nach `mmu::reinstall`, **vor** Rahmen- und Heap-Init, eine 2-MiB-Identitätsabbildung des ganzen DRAM (EL0 RW, PXN) in TTBR0 und lassen sie den ganzen Rest des Boots aktiv; `spike.early-replace[-notlbi]` holt vorher die Stage-1-Boot-Identitätsabbildung (1-GiB-Blöcke, EL1-only) zurück nach TTBR0, liest einmal über sie (TLB gefüllt) und tauscht dann — das ist der Zustand vom 2026-06-16.
7. **Q4** Wurzel mit Seiten `0x1000` EL0 RW, `0x2000` EL0 RO, `0x3000` EL1-only, `0x4000` ungültig; `ldtr`/`sttr`/`ldr` von EL1 mit Fehlerabfang (Haken setzt `ELR += 4`, wie `probe_read32`). PAN wird nur geprüft, wenn `MMFR1.PAN ≠ 0`.
8. **Q3** Vorlage = Pool-Abbildung (2 MiB, **`nG = 1`**); zwei Wurzeln A/B nach Plan §3.2: eigene L0/L1/L2₀/L3₀, alles übrige zeigt auf die geteilten Tabellen der Vorlage, Seite 0 privat (A bzw. B). Je Folge 16 Ausflüge A,B,A,B,… zweimal: einmal Einstieg im geteilten Code (VA = PA), der `[0]` liest und `blr 0x100` in den privaten Code springt; einmal **Direkteinstieg** per `eret` auf die private VA `0x180` (genau die QUIRK-001-Lage: gleiche VA, andere PA, Code). Geprüft: Tag aus dem Code, Datenwort bei VA 0, Zähler in geteiltem Speicher. Folgen: `sw-bracket` (Produktionsweg: `restore_boot_ttbr0` → UART-Zeile → `install_ttbr0` → `eret`), `sw-arch` (`dsb; msr leer; tlbi vmalle1; dsb ish; isb; msr neu; isb; eret` — alles in einem `asm`-Block, kein Gerätezugriff), `sw-tlbi-eret` (`msr neu; isb; tlbi vmalle1; dsb ish; eret`), `sw-notlbi` (`msr neu; isb; eret`, ASID 0), `sw-asid` (ASID 1/2 in `TTBR0[63:48]`, `TCR.AS = 0`, nur `msr; isb; eret`; vorab einzeln der getaggte `msr` mit Meldung).

**Läufe** (Skripte in `method/`, Laufzeit begrenzt per `sleep`+`kill`, der Host hat kein `timeout`):

| Abbild | Maschine | Boot-Optionen | Läufe | Log |
|---|---|---|---|---|
| Pi-Abbild (Release, `sha256 25b00e36…6b5e`) | `-M raspi4b` | `serial=on spike.nopause` | 3 | `serial/raspi4b-release-run{1,2,3}.log` |
| dasselbe | `-M raspi4b` | `+ spike.early`, `spike.early-notlbi`, `spike.early-replace`, `spike.early-replace-notlbi` | je 1 | `serial/raspi4b-release-spike.early*.log` |
| dasselbe | `-M raspi4b` + QMP | `serial=on` (Blättern an) | 1 | `serial/raspi4b-release-pause.log`, `shots/raspi4b-release-page1.png` |
| `board-qemu-virt` (dev-Profil) | `-M virt,gic-version=2 -cpu cortex-a72 -m 272M -device ramfb …` | — | 3 | `serial/virt-debug-run{1,2,3}.log` |
| dasselbe, `SPIKE_ARGS=<option>` beim Bau | `virt` | die vier `spike.early*` | je 1 | `serial/virt-debug-spike.early*.log` |

Alle 15 Läufe sind bis auf Adressen und Zeitwerte zeilengleich (je 6 × „OK“ in Q2, je 4 Folgen 32/32, `sw-notlbi` je 16/32, alle `early`-Läufe „survived the whole boot“).

---

## 2. Q1 — Identifikationsregister an EL1 [QEMU gemessen]

`serial/raspi4b-release-run1.log:37-44`, `serial/virt-debug-run1.log:56-63` (beide Boards identisch bis auf `PFR0` und `SCTLR`):

| Register | QEMU (beide) | Dekodiert | A72-TRM-Erwartung (V0a) |
|---|---|---|---|
| `MIDR_EL1` | `0x410FD083` | Arm, Teil `0xD08` (A72), r0p3 | r0p3 ✓ |
| `ID_AA64MMFR0_EL1` | `0x1124` | PARange 4 (44 Bit), **ASIDBits 2 = 16 Bit**, TGran4 0 (vorhanden) | `0x00001124` (TRM S. 4-116, Registerübersicht Zeile „ID_AA64MMFR0_EL1 … 0x00001124“) ✓ |
| `ID_AA64MMFR1_EL1` | `0` | HAFDBS 0, VMIDBits 0, VH 0, **HPDS 0, PAN 0**, XNX 0 | RES0 ✓ |
| `ID_AA64MMFR2_EL1` | `0` | **UAO 0, BBM 0**, CnP 0, E0PD 0 | im TRM nicht aufgeführt (v8.0) ✓ |
| `CTR_EL0` | `0x8444C004` | IminLine/DminLine 64 B, L1Ip 3 (PIPT), **IDC 0, DIC 0**, CWG/ERG 4 | `0x8444C004` ✓ |
| `CLIDR_EL1` | `0x0A200023` | L1 getrennt I/D, L2 vereinheitlicht; LoUU 1, LoC 2, LoUIS 1 | `0x0A200023` ✓ |
| `CCSIDR_EL1` | L1D 32 KiB **4-fach**, L1I 48 KiB 3-fach, L2 1024 KiB 16-fach, je 64 B | — | L1D laut TRM **2-fach** (`0x701FE00A`), L1I 3-fach, L2 je nach Bestückung (1 MiB = `0x707FE07A`) — **QEMU weicht bei L1D ab** |
| `TCR_EL1` | `0x2_B510_3510` | `AS = 0`, `A1 = 0`, HPD0/1 = 0 | wie `boot.rs` `TCR_VALUE` ✓ |

**Ergebnis:** QEMUs `cortex-a72` meldet die TRM-Werte für alle Speicherverwaltungsregister; einzige Abweichung ist die L1D-Assoziativität (QEMU 4-fach, TRM 2-fach) — für ARCH03 belanglos (Cache-Pflege nach VA, Zeilengröße 64 B stimmt). Die Pi muss die L2-Größe des BCM2711 (1 MiB erwartet) und die L1D-Assoziativität zeigen.

## 3. Q2 — VA = PA in TTBR0 neben dem TTBR1-Alias („Wand 3“) [QEMU gemessen]

| Variante | Tabellen (`raspi4b` / `virt`) | Ergebnis (beide Boards, je 3 Läufe) | Beleg |
|---|---|---|---|
| `a2m-tlbi` — Pools, 2-MiB-Blöcke, `msr`+`tlbi` | 3 / 4 | OK | `raspi4b-release-run1.log:48-50` |
| `a4k-tlbi` — Pools, nur 4-KiB-Seiten | 479 / 132 | OK | `:51-53` |
| `a2m-notlbi` | 3 / 4 | OK | `:54-55` |
| `a4k-notlbi` | 479 / 132 | OK | `:56-57` |
| `adram-tlbi` — ganzes DRAM inkl. Kernelbild und Heap (ADR-010-Abbildung) | 3 / 3 | OK | `:58-59` |
| `adram-notlbi` | 3 / 3 | OK | `:60-61` |
| `spike.early` / `-notlbi` — DRAM-Identität ab vor `frames::init`/`heap::init` bis zum Spike | — | Boot vollständig (Rahmen-, Heap-, FB-, Pool-, GIC-, Timer-Init, SD-Erkennung, USB-Init, Begrüßungsbildschirm) | `raspi4b-release-spike.early.log:20-21,38`; `virt-debug-spike.early.log` |
| `spike.early-replace` / `-notlbi` — exakt ADR-010: Boot-Identität (1 GiB, EL1-only, TLB gefüllt) → 2-MiB-EL0-Identität | — | Boot vollständig | `raspi4b-release-spike.early-replace.log:20-22,39`; `virt-debug-spike.early-replace-notlbi.log:20-22,58` |

„OK“ heißt: der erste Kernel-Store nach dem Einbau (Heap-`Box`, Store über den Alias, `static`) kommt durch; EL1 liest über VA = PA denselben Wert; EL0 liest den Kernelwert und schreibt einen neuen, den der Kernel über den Alias sieht; eine zweite Runde ebenso. Der Kernel-Heap liegt heute bei `0x3BE0_0000` (`raspi4b`) bzw. `0x5061_7000` (`virt`) in der OS-Reserve (`serial/*:13-15`), 2026-06-16 lag er bei `0x4100_0000` am TT-RAM-Anfang — die DRAM-Varianten bilden ihn jedenfalls mit ab.

**Ergebnis:** Auf QEMU 11.0.2 hängt **keine** Variante, auch nicht die exakte Nachstellung vom 2026-06-16 (Austausch einer gefüllten 1-GiB-EL1-Abbildung gegen die 2-MiB-EL0-Abbildung, mit und ohne TLBI). Zusammen mit V0a (B2.11: Rechte-Unterschiede zwischen Aliasen sind erlaubt) ist Wand 3 auf QEMU 11.0.2 weder architektonisch noch empirisch vorhanden. **[Schluss]** Der damalige Hänger war entweder ein QEMU-11.0.0-Fehler oder eine Folge des damaligen Kerns (der Heap dereferenzierte seinerzeit noch PAs über die niedrige Hälfte, ADR-010 „Stage 0“); welches von beiden, lässt sich ohne 11.0.0 nicht mehr trennen.

**Leistung 2 MiB vs. 4 KiB:** QEMU-Werte (`raspi4b` 166–201 ns/Load mit 2-MiB-Blöcken, 144–164 ns mit 4-KiB-Seiten; `virt` 145–153 / 125–134 ns) messen nur QEMUs Software-TLB und sind **nicht** aussagekräftig. Die Pi-Zahl entscheidet V0a §11 Punkt 4.

## 4. Q3 — Wurzeln je `memspace` über geteilten Tabellen; Wechselfolgen [QEMU gemessen]

Aufbau wie Plan §3.2: zwei Wurzeln teilen alle Tabellen unterhalb von L1/L2₀ (die Pool-Blöcke), Seite 0 privat — mit **Code** an derselben VA (Trampolin-Fall des Plans, §3.1/§3.14). Ergebnisse je Board identisch in allen 3+4 Läufen (`raspi4b-release-run1.log:110-119`, `virt-debug-run1.log:129-138`):

| Folge | geteilter Einstieg | Direkteinstieg `eret` → `0x180` | Bewertung |
|---|---|---|---|
| `sw-bracket` — Produktion: `restore_boot_ttbr0` (leer + `tlbi vmalle1`) → UART-Zeile → `install_ttbr0` → `eret` | 16/16 | 16/16 | funktioniert (erwartet) |
| `sw-arch` — `dsb; msr leer; tlbi vmalle1; dsb ish; isb; msr neu; isb; eret`, **ohne** Gerätezugriff | 16/16 | 16/16 | **QUIRK-001 nicht reproduzierbar**; die UART-Klammer ist auf 11.0.2 entbehrlich |
| `sw-tlbi-eret` — `msr neu; isb; tlbi vmalle1; dsb ish; eret` | 16/16 | 16/16 | **„kein TLBI direkt vor `eret`“ (Wand 1) gilt auf 11.0.2 nicht** |
| `sw-notlbi` — `msr neu; isb; eret`, ASID 0 | 8/16 | 8/16 | jeder Wechsel nach dem ersten liefert Tag **und** Datenwort der *vorigen* Wurzel (`got a/a0a0a0a0`) — QEMU modelliert den TLB architekturgetreu: ohne TLBI und ohne ASID bleibt der alte Eintrag gültig. Kein Hänger, kein Abbruch |
| `sw-asid` — ASID 1/2, `nG = 1`, `TCR.AS = 0`, nur `msr; isb; eret` | 16/16 | 16/16 | **ASID-getaggter `msr` hängt nicht mehr** (Wand 2), Wechsel ohne TLBI korrekt |

**Ergebnis:** Auf QEMU 11.0.2 funktionieren die Architekturfolge aus V0a (TLBI vor dem Wurzelwechsel, Kontext-Synchronisation durch `eret`), TLBI unmittelbar vor `eret` und ASIDs; die QUIRK-001-Klammer ist nicht mehr nötig. Notwendig ist — architektonisch wie auf QEMU — **entweder** TLBI **oder** eine eigene ASID je Wurzel (sonst falsche Seite 0, `sw-notlbi`).

**Grenze dieser Messung:** Der Original-Repro von QUIRK-001 war der volle `Pexec`/`Pterm`-Weg mit dem Phase-06-D3-Selbsttest. Ihn mit entfernter Klammer nachzufahren, scheiterte am Selbsttest selbst: `selftest_pexec::run_pexec_test` meldet heute `FAIL: parent loader: SegmentVaOutOfRange` (handgebautes ELF bei VA `0x100`, unverdrahteter Selbsttest — Merkregel „Unverdrahtete Selbsttests verrotten still“); der Versuch wurde zurückgenommen. **Indizienbeleg** aus dem Repo: In `finish_terminate` (`kernel/src/arch/aarch64/exceptions.rs:1376-1381`) stehen `restore_boot_ttbr0` und `install_ttbr0(resume.parent_l0_pa)` seit Commit `a3c4329` (2026-06-19) **ohne** UART-Zeile dazwischen (die Zeile `[pterm] resume parent` verschwand dort, `git log -S`), obwohl der Kommentar darüber (`:1359-1375`) sie noch als „LOAD-BEARING“ beschreibt. Ob dieser Pfad (Rückkehr zu einem EL0-Elternprozess) seither auf QEMU regelmäßig durchlaufen wurde, habe ich nicht geprüft — es ist ein Indiz, kein Beleg.

## 5. Q4 — `ldtr`/`sttr` an EL1 und ihr Abbruch [QEMU gemessen]

`raspi4b-release-run1.log:62-75`, `virt-debug-run1.log:81-94` (identisch bis auf Kerneladressen):

| Zugriff (EL1) | Ergebnis | `ESR_EL1` | DFSC | WnR | `FAR_EL1` |
|---|---|---|---|---|---|
| `ldtr` EL0-RW-Seite `0x1000` | Wert `0x1111` | — | — | — | — |
| `sttr` EL0-RW-Seite `0x1008` | Store sichtbar über Alias (`0x5555`) | — | — | — | — |
| `ldtr` EL0-RO-Seite `0x2000` | Wert `0x2222` | — | — | — | — |
| `sttr` EL0-RO-Seite `0x2000` | Abbruch | EC `0x25`, ISS `0x4F` | `0x0F` Permission L3 | 1 | `0x2000` |
| `ldtr` EL1-only-Seite `0x3000` (`AP=00`) | Abbruch | EC `0x25`, ISS `0x0F` | `0x0F` Permission L3 | 0 | `0x3000` |
| `ldtr` `0x4000` (L3 ungültig) | Abbruch | ISS `0x07` | `0x07` Translation L3 | 0 | `0x4000` |
| `ldtr` `0x20_0000` (L2 ungültig) | Abbruch | ISS `0x06` | `0x06` Translation L2 | 0 | `0x20_0000` |
| `ldtr` `1<<38` (L1 ungültig) | Abbruch | ISS `0x05` | `0x05` Translation L1 | 0 | `0x40_0000_0000` |
| `ldtr` `1<<39` (L0 ungültig) | Abbruch | ISS `0x04` | `0x04` Translation L0 | 0 | `0x80_0000_0000` |
| `ldtr` Kernel-`static` (TTBR1, `AP=00`, 2-MiB-Block) | Abbruch | ISS `0x0E` | `0x0E` Permission L2 | 0 | Kernel-VA |
| `ldr` (privilegiert) EL0-RO-Seite `0x2000` | Wert `0x2222` | — | — | — | — |

`ELR_EL1` zeigt in jedem Fall auf den `ldtr`/`sttr`-Befehl selbst (z. B. `0xffff00000008f79c`), d. h. die Fixup-Tabelle nach PC (V0a, Plan Frage 10) ist der richtige Unterscheider. PAN: `MMFR1.PAN = 0`, nicht prüfbar — der privilegierte `ldr` auf eine Anwenderseite gelingt, wie erwartet. **Ergebnis:** QEMU verhält sich exakt wie in V0a §4/§6 aus dem ARM ARM abgeleitet; das geplante `uaccess`-Primitiv (`ldtr/sttr` + PC-Fixup) ist auf QEMU erprobt.

## 6. Q5 — Bildschirmspeicher gegen die 2-GiB-/32-Bit-Grenze [QEMU gemessen]

| Board | FB-PA | Lage | < 2 GiB | Beleg |
|---|---|---|---|---|
| `raspi4b` | `0x3C10_0000..0x3C35_8000` (640×480, 2 Seiten, 2400 KiB) | VideoCore-Speicher oberhalb `dram.end = 0x3C00_0000` | ja | `raspi4b-release-run1.log:45-46` |
| `virt` | `0x5081_7000..0x5100_0000` (1920×1080, 8100 KiB) | OS-Reserve am DRAM-Ende | ja | `virt-debug-run1.log:64-65` |

**Nebenbefund:** Auf `raspi4b` beginnt der Bildschirm 1-MiB-, aber nicht 2-MiB-ausgerichtet; eine VA = PA-Abbildung in TTBR0 als 2-MiB-Block griffe in den VideoCore-Speicher davor. Für die Anwender-Abbildung (Plan §3.1, „Bildschirmspeicher Normal-NC, PROT_G“) daher 4-KiB-Seiten verwenden **[Schluss]**. Die Pi-Lage (bei `gpu_mem=256` erwartet unterhalb 1 GiB) liefert der Pi-Lauf.

---

## 7. Pi-Lauf — Anleitung für den Anwender

**Abbild:** `kernel8-spike.img` (Release, `board-rpi4`, Feature `arch03-spike`), nicht im Repo (Bilddateien sind ausgeschlossen). Liegt im Sitzungs-Scratchpad unter
`/private/tmp/claude-501/-Volumes-Daten-Development-PiRusTOS/b705aa1b-c171-48ba-ac49-a9f08dec4fa1/scratchpad/arch03-0b/kernel8-spike.img`
`sha256 25b00e36bd71ced9d466b0a537fc3362ab9eda8cb65e4b317688f8c718a16b5e` — genau dieses Abbild lief für §2–§6 auf QEMU `raspi4b`.

1. **Sichern:** Auf der Boot-Partition der Karte die vorhandene `kernel8.img` umbenennen (z. B. `kernel8.ofam`). Sonst nichts an der Karte ändern.
2. `kernel8-spike.img` als `kernel8.img` auf die Boot-Partition kopieren.
3. `cmdline.txt` auf der Boot-Partition: eine Zeile `serial=on` (ohne sie entfällt die UART-Last der Folge `sw-bracket`; die Bildschirmausgabe kommt in jedem Fall). Gibt es die Datei schon, nur das Wort `serial=on` ergänzen.
4. USB-Tastatur angesteckt lassen, einschalten. Nach dem Begrüßungsbildschirm (≈ 15 s oder Taste) erscheint `ARCH03 stage-0b spike on rpi4 …`. Bei 1920×1080 passt alles auf eine Seite; sonst steht unten `--- photograph this page, then press a key ---` → fotografieren, Taste, weiter.
5. **Fertig**, wenn `SPIKE DONE. Photograph the screen. The system stops here.` erscheint: fotografieren (ganzer Bildschirm, lesbar), dann ausschalten.
6. **Bleibt der Bildschirm bei einer Zeile mit `...` am Ende stehen** (≥ 1 Minute), ist das ein Hänger in genau diesem Schritt — **das ist ein Ergebnis**: fotografieren, ausschalten. Optional zweiter Lauf mit `serial=on spike.skip=<id>` (z. B. `spike.skip=sw-asid`; `<id>` = das Wort vor dem Doppelpunkt), um die restlichen Schritte zu sehen.
7. **Optional** (ADR-010-Nachstellung auf Hardware): je ein weiterer Lauf mit `cmdline.txt` = `serial=on spike.early-replace` bzw. `serial=on spike.early-replace-notlbi`. Erfolgreich, wenn die zweite Zeile `early (ADR-010 replay, replacing boot map, …): … survived the whole boot -> OK` lautet. Bleibt der Bildschirm schwarz (kein Begrüßungsbildschirm), hängt die Nachstellung — auch das fotografieren/melden.
8. **Zurück:** `kernel8.img` löschen, `kernel8.ofam` zurück in `kernel8.img` umbenennen, `spike.*` aus `cmdline.txt` entfernen (`serial=on` darf bleiben).

### Ergebnistabelle Pi (vom Anwender-Foto auszufüllen)

| Zeile auf dem Schirm | QEMU `raspi4b` | Erwartung Pi (TRM/Architektur) | Pi gemessen |
|---|---|---|---|
| `Q1 MIDR` | `410fd083` r0p3 | `410fd083` (A72 r0p3) | |
| `Q1 MMFR0` ASIDBits | 2 (16 Bit) | 2 | |
| `Q1 MMFR1` PAN / HPDS / HAFDBS | 0 / 0 / 0 | 0 / 0 / 0 | |
| `Q1 MMFR2` UAO / BBM | 0 / 0 | 0 / 0 | |
| `Q1 CTR` IDC / DIC | 0 / 0 | 0 / 0 | |
| `Q1 CLIDR` L1D / L1I / L2 | 32K 4w / 48K 3w / 1024K 16w | 32K **2w** / 48K 3w / 1024K 16w | |
| `Q5 FB PA` | `0x3c100000..0x3c358000` | < `0x4000_0000` [Schluss] | |
| `Q5 DRAM … ST … TT …` | `0x0..0x3c000000`, TT bis `0x3be00000` | Pool wie Plan Frage 2 | |
| `a2m-tlbi` … `adram-notlbi` (6 Zeilen) | OK | OK (B2.11) | |
| `perf a2m-tlbi` / `perf a4k-tlbi` (ns/load) | QEMU-Wert ohne Aussage | 2 MiB ≤ 4 KiB erwartet | |
| `Q4` 11 Zeilen | wie §5 | wie §5 | |
| `sw-bracket` | 16/16 + 16/16 | 16/16 + 16/16 | |
| `sw-arch` | 16/16 + 16/16 | 16/16 + 16/16 | |
| `sw-tlbi-eret` | 16/16 + 16/16 | 16/16 + 16/16 | |
| `sw-notlbi` | 8/16 + 8/16 falsch, kein Abbruch | falsch **oder** TLB-Konfliktabbruch (`DFSC 0x30`), beides architekturkonform | |
| `sw-asid` | 16/16 + 16/16 | 16/16 + 16/16 | |
| optional `spike.early-replace[-notlbi]` | OK / OK | OK / (ohne TLBI architektonisch nicht garantiert) | |

---

## 8. Folgen für den Plan (als „0b:“ in `arch03-implementation-plan.md` eingetragen)

1. **Wand 3 / Rückfallebene E18:** auf QEMU 11.0.2 nicht reproduzierbar (§3) → Stufe 1 kann die Pools **beim Booten** abbilden; „Abbildung bei Allokation“ bleibt nur Rückfall für den Fall, dass die Pi widerspricht.
2. **QUIRK-001-Klammer:** auf QEMU 11.0.2 entbehrlich (§4, `sw-arch`/`sw-tlbi-eret`); Stufe 1 kann die Architekturfolge `TLBI; DSB ISH` + `eret` verwenden — **nach** Bestätigung auf der Pi. Invariante I9 („kein TLBI unmittelbar vor `eret`“) ist auf QEMU 11.0.2 hinfällig.
3. **ASIDs (E12, Stufe 4):** auf QEMU 11.0.2 funktionsfähig (16 Bit gemeldet, getaggter `msr` hängt nicht, Wechsel ohne TLBI korrekt). Die Begründung „nur Hardware, weil QEMU hängt“ entfällt; Stufe 4 kann auf QEMU entwickelt werden.
4. **Q3-Pflicht:** ohne TLBI und ohne ASID liefert der Wechsel nachweislich die alte Seite 0 (`sw-notlbi`) — die prozesseigene Seite 0 braucht in jedem Wechsel TLBI oder ASID.
5. **Bildschirm:** < 2 GiB auf beiden QEMU-Boards; auf `raspi4b` nicht 2-MiB-ausgerichtet → 4-KiB-Abbildung.
6. **`uaccess`:** Fehlerbild wie V0a, Fixup nach PC tragfähig; PAN fehlt (QEMU wie TRM).
7. **Tabellenbudget (Frage 14):** Pi-Pool mit 2-MiB-Blöcken 3 Tabellen, mit 4-KiB-Seiten 479 (≈ 1,9 MiB) — gemessen auf `raspi4b`.

Alle Punkte 1–4 stehen unter dem Vorbehalt des Pi-Laufs; QUIRK-001/ADR-010 in `docs/MEMORY.md` werden erst danach nachgetragen (Vorschlag, nicht von mir geändert).

## 9. Beobachtungen außerhalb des Auftrags (je eine Zeile, unverändert gelassen)

- `kernel/src/arch/aarch64/exceptions.rs:1359-1375`: Kommentar nennt eine „LOAD-BEARING“ UART-Zeile zwischen `restore_boot_ttbr0` und `install_ttbr0`, die im Code seit `a3c4329` fehlt.
- `kernel/src/selftest_pexec.rs`: `run_pexec_test` scheitert mit `SegmentVaOutOfRange` (handgebautes ELF bei VA `0x100`); unverdrahtet, daher unbemerkt.
- QEMU `cortex-a72` meldet L1D 4-fach (`CCSIDR`), der A72-TRM 2-fach — falls künftig set/way-Pflege nach `CCSIDR` gerechnet wird, nicht an QEMU kalibrieren.


## Pi-Ergebnis 2026-10-05 (Lead, aus den Fotos des Anwenders)

Abschrift: `measurements/measurement-2026-10-05-arch03-0b/pi/PI-RESULTS.TXT`,
Fotos daneben. Die echte Pi 4 bestaetigt QEMU 11.0.2 in jedem Punkt:
A72 r0p3, 16-Bit-ASIDs, kein PAN/UAO/BBM; L1D 2-fach (wie TRM, QEMU meldete
4-fach); alle sechs Abbildungsvarianten OK (ADR-010 „Wand 3“ tritt auch auf
Hardware nicht auf); Umschalten: Klammer, Architekturfolge, TLBI vor `eret`
und ASID je 16/16+16/16, ohne TLBI/ASID 8/16 falsch (erwartet); `ldtr/sttr`-
Fehlerbild identisch; Bildschirmspeicher `0x3dbf5000..0x3ebc7000` (unter
1 GiB, ausserhalb des ARM-DRAM); 2-MiB-Bloecke 34 ns/Load gegen 4 KiB 36 ns.
Folge fuer den Plan: Pools beim Booten abbilden (E18 entfaellt), QUIRK-001-
Klammer und Invariante I9 koennen fallen, ASIDs sind auf QEMU und Pi nutzbar.
