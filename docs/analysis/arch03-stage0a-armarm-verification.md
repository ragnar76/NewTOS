# ARCH03 Stufe 0a — Track-B-Nachweis gegen ARM ARM, Cortex-A72-TRM und AAELF64

Stand 2026-10-04 · Researcher · **Dokument, kein Code.** Prüft jede als „[Track B offen]“ gekennzeichnete bzw. „nach Gedächtnis“ formulierte Architekturaussage aus `docs/analysis/arch03-implementation-plan.md` (im Folgenden **Plan**) und `docs/analysis/arch03-freemint-memory-model.md` §E (**S1**) gegen die gepinnten Primärquellen.

## 0. Quellen und Zitierweise

| Kürzel | Dokument (gepinnt in `references/README.md`) | Ausgabe |
|---|---|---|
| **ARM ARM** | `references/arm-arm/DDI0487M_d_a-profile_architecture_reference_manual.pdf` | DDI 0487 **M.d** (nicht M.c wie im Plan Stufe 0a und QUIRK-003 angenommen) |
| **TRM** | `references/arm-arm/cortex_a72_mpcore_trm_100095_0003_06_en.pdf` | Cortex-A72 MPCore r0p3, Issue 06 |
| **AAELF64** | `references/arm-arm/aaelf64.pdf` | ELF for the Arm 64-bit Architecture, 2025Q4 |

- **Seitenangaben** sind die auf den Seiten gedruckten Fußzeilen (`D8-7742`, `4-116`, `40`); sie sind in diesen PDFs identisch mit der PDF-Seitennummer. ARM-ARM-Regeln werden zusätzlich mit ihrer **Absatzkennung** zitiert (sechsbuchstabig, z. B. `RVWLLR`) — die bleibt auch bei Layoutänderungen stabil.
- Extraktion: `pdftotext -layout` in den Sitzungs-Scratchpad (nicht im Repo). Alles Zitierte wurde in dieser Sitzung im extrahierten Text gelesen.
- **Urteile:** **bestätigt** · **korrigiert** (Plan sagt etwas Falsches/Unpräzises, wird geändert) · **n/a** (nicht anwendbar) · **offen** (aus den gepinnten Quellen nicht entscheidbar → Stufe 0b bzw. weitere Quelle). **[gelesen]** = Wortlaut/Tabelle in der Quelle; **[Schluss]** = aus Gelesenem gefolgert, nicht wörtlich dort.
- „Plan ändern“ = ja → Änderung ist in `arch03-implementation-plan.md` mit `0a:` eingearbeitet.

**Bilanz (68 Aussagenzeilen in §1–§8, dazu 10 Einordnungszeilen in §9):** 60 **bestätigt** (davon 12 reine Zusatzfakten ohne Gegenstück im Plan: A6, C7, C12, C13, D5, D6, F6, F8, G3, H7, H8, G1-Laufzeitvorbehalt), **3 korrigiert** (C2 TLBI auch bei Erweiterung gültig→gültig; C3 Break-before-make-Liste; C10 „kein TLBI vor `eret`“ ist kein Architekturgebot), **1 n/a** (A10 Stage 2), **4 offen** (G4 A72-Errata, H4 Relokationstypen des Linkers, H5 `Elf64_Rela`, H6 Dynamic Section/`ET_DYN`/`DT_*`). §9: 3 Architekturregeln (I1, I2, I8), 4 reine QEMU-Effekte (I3, I5, I6, I7), 1 korrigierte Begründung (I4), 2 Info/n/a (I9, I10).

---

## 1. Deskriptoren (VMSAv8-64, Stage 1, 4-KiB-Granule)

Die Plan-Stufen 1–3 benutzen ausschließlich **Stage-1-Deskriptoren** des EL1&0-Regimes. Stage 2 ist ausgeschaltet (A10).

| ID | Aussage | Urteil | Beleg | Plan ändern |
|---|---|---|---|---|
| A1 | Bitlagen in Block-/Seitendeskriptoren: `AP[2]`=Bit 7, `AP[1]`=Bit 6, `SH`=[9:8], `AF`=Bit 10, `nG`=Bit 11, `AttrIndx`=[4:2], `PXN`=Bit 53, `UXN`=Bit 54; Bit 0 = gültig; Bit 1 = Tabelle (L0–L2) bzw. Seite (L3) | **bestätigt** [gelesen] | ARM ARM Figure D8-16 und Table D8-52 (S. D8-7741…D8-7744); Table D8-48 (D8-7735). Entspricht `tos-api/src/bdos/process/mmu.rs:91-111` (`NT/mmu.rs`) | nein (Plan nennt „Linux-Querverweis“ → jetzt Primärquelle) |
| A2 | `AP[2:1]`: `00` = EL1 RW / EL0 nichts; `01` = EL1+EL0 RW; `11` = EL1+EL0 nur lesen; (`10` = EL1 nur lesen, EL0 nichts) | **bestätigt** [gelesen] | Table D8-63 (D8-7756) | nein |
| A3 | „PXN = 1 überall“ auf Anwenderseiten | **bestätigt, Begründung präzisiert** [gelesen] | `RVWLLR` (D8-7758): *„if UnprivWrite is present, then PrivExecute is removed“* — bei `AP=01` ist EL1-Ausführung ohnehin entfernt (Table D8-65, D8-7759/7760: Zeilen `AP=01` haben nie `PrivExecute`). PXN wirkt real nur bei `AP=11` (nur lesbare Anwenderseiten, `PROT_PR`-Nicht-Besitzer, Kernel-veröffentlichte `UserRoNx`): dort wäre ohne PXN EL1-Ausführung erlaubt (Zeile `UXN=0 PXN=0 AP=11`). Zeile `UXN=0 PXN=1 AP=11` = `PrivRead, UnprivRead, UnprivExecute` (D8-7759/7760) ist genau die Plan-Vorgabe für `PROT_PR` | ja (Begründung in §3.3) |
| A4 | `UXN=0` für Anwenderspeicher (68k kennt kein NX); Ausführen aus beschreibbarem EL0-Speicher erlaubt | **bestätigt** [gelesen] | Table D8-65 (`UXN=0 AP=01` → `UnprivWrite, UnprivExecute`). `SCTLR_EL1.WXN` (TRM Table 4-44, S. 4-126) würde das global verbieten; bleibt aus (Boot-Wert) | nein |
| A5 | `nG`: `0` = global (gilt für alle ASIDs), `1` = nur aktuelle ASID | **bestätigt** [gelesen] | `RQGKGF` (D8-7854); Bit 11 nur bei Regimen mit zwei Privilegstufen (Table D8-52, D8-7744). **Ergänzung:** Einträge **nicht-finaler** Lookup-Ebenen gelten **immer** als nicht-global, unabhängig von `nG` (`RCHVGJ`, D8-7854) | ja (Frage 15, Stufe 4) |
| A6 | `AF` muss in jedem gültigen Deskriptor `1` sein | **neu / bestätigt** [gelesen] | Fehlerprüfreihenfolge Schritt 9 (D8-7845): AF=0 ohne HW-Verwaltung → Access-flag-Fault. Der A72 hat **keine** HW-Verwaltung: TRM Table 4-24 `ID_MMFR2_EL1.HWAccFlg = 0x0 „Not supported“` (S. 4-100). Repo setzt `D_AF` bereits (`tos-api/src/bdos/process/mmu.rs:201-206`, `descriptor_bits`, gelesen) | ja (Invariante in §3.3) |
| A7 | `SH`: Normal-WB = Inner Shareable (`0b11`); Kernel-Alias (TTBR1) und Anwender-Alias (TTBR0) müssen **Speichertyp, Shareability und Cacheability** gleich haben, **Rechte** (AP/PXN/UXN) dürfen abweichen | **bestätigt** [gelesen] | `SH`: Table D8-52 (D8-7744). Alias-Regel: ARM ARM B2.11 „Mismatched memory attributes“ (S. B2-331): nur *Memory type, Shareability, Cacheability (und XS)* zählen als „attributes“ — AP/XN gehören **nicht** dazu. Kernel-Alias Normal/IS: `KA/mmu.rs:179,284,511`; Anwender: `tos-api/src/bdos/process/mmu.rs:201-206` (`D_SH_INNER`, `D_ATTRIDX_NORMAL`, `D_PXN`, kein `nG`) — gleich | nein |
| A8 | Tabellendeskriptor: `APTable`=[62:61], `UXNTable`=Bit 60, `PXNTable`=Bit 59 (Stage 1, zwei Privilegstufen); Wirkung auf alle tieferen Ebenen | **bestätigt** [gelesen] | Figure D8-13 / Table D8-50 (D8-7737/7738); Wirkung: Table D8-64 (D8-7757), `RRLQFP/RWSFPB/RNJJNG` (D8-7758/7759). **Ergänzung (neu):** auf dem A72 sind die Hierarchiebits **nicht abschaltbar** — `TCR_EL1.HPD0/HPD1` (Bits 41/42) sind im A72-TRM „Reserved, RES0“ (Table 4-53, S. 4-147: [63:39] RES0), `ID_AA64MMFR1_EL1` ist laut TRM `RES0` (Memory-Map S. 4-364); in der Architektur ist `HPDS=0` zulässig (D24-9254). Folge: **Bits [63:59] aller Tabellendeskriptoren müssen 0 sein** (oder bewusst genutzt werden). Änderung dieser Bits verlangt Break-before-make samt TLBI aller Ebenen (`IYQWJF` D8-7757, `IZGSNV` D8-7759) | ja (§3.2 Invariante) |
| A9 | Blöcke: L1 = 1 GiB (n=30), L2 = 2 MiB (n=21); L0-Block gibt es bei 4 KiB nur mit `TCR_EL1.DS=1` (FEAT_LPA2, nicht A72); L3 Bit 1 = 0 ist „reserved, treated as invalid“. Für **EL0-Zugriff ändert sich bei Block statt Seite nichts**: dieselben Felder `AP/UXN/PXN/nG/AF/SH/AttrIndx` an denselben Bitlagen | **bestätigt** [gelesen] | Figure D8-14 (D8-7740), Table D8-48 (D8-7735), Table D8-52 (D8-7743: „level 0 Block … Effective value of TCR_ELx.DS is 1“). Permission-Fault meldet die Ebene des Block-/Seitendeskriptors (`RCSRQR` D8-7841). **A72-Detail [gelesen]:** L1-TLBs cachen nativ 4 KiB/64 KiB/1 MiB; Blöcke > 1 MiB belegen je 1-MiB-Bereich einen L1-Eintrag, der L2-TLB kann 2 MiB und 1 GiB (TRM 5.2.1–5.2.3, S. 5-277) | nein (Spike-Frage „2-MiB-Blöcke“ bleibt Leistungsfrage) |
| A10 | Stage-2-Deskriptoren (S2AP/XN[1:0]) | **n/a** | Table D8-51 (D8-7739). Boot setzt nur `HCR_EL2.RW` (`kernel/src/arch/aarch64/boot.rs:303-304` gelesen) → `VM=0`, Stage 2 aus | nein |

---

## 2. TTBR0/TTBR1, `TCR_EL1`, ASID

| ID | Aussage | Urteil | Beleg | Plan ändern |
|---|---|---|---|---|
| B1 | Obere VA-Hälfte → TTBR1, untere → TTBR0, Breite je Hälfte `64−TnSZ`; `T0SZ=T1SZ=16` ⇒ 48 Bit, Start bei L0; ungültige obere Bits ⇒ Translation-Fault Level 0 | **bestätigt** [gelesen] | `T0SZ` D24-9923, `T1SZ` D24-9921; `RVZZSZ` (D8-7840) | nein |
| B2 | `TG0=0b00` = 4 KiB, `TG1=0b10` = 4 KiB (asymmetrisch) | **bestätigt** [gelesen] | `TG0` D24-9922, `TG1` D24-9919 (TRM Table 4-53 zeigt dasselbe: `[31]` RES1, `[30]` TG1 `0`=4 KiB, S. 4-147) | nein |
| B3 | `IPS=0b010` (40 Bit) ≤ unterstützte PA-Breite des A72 | **bestätigt** [gelesen] | IPS-Tabelle D24-9918; A72 `ID_AA64MMFR0_EL1.PARange = 0x4` = 44 Bit (TRM Table 4-35, S. 4-116) | nein |
| B4 | `TCR_EL1.A1=0` ⇒ ASID aus `TTBR0_EL1[63:48]`, gilt auch für TTBR1-Übersetzungen | **bestätigt** [gelesen] | `A1` D24-9921; `RSCBSR` (D8-7854); TTBR0-Layout D24-10041/10042, TRM Figure 4-40/Table 4-56 (S. 4-153) | nein |
| B5 | **ASID-Breite:** 8 Bit solange `TCR_EL1.AS=0` (Bit 36), 16 Bit bei `AS=1`; Maximum meldet `ID_AA64MMFR0_EL1.ASIDBits` | **bestätigt** [gelesen] | `AS` D24-9917 (Feld existiert nur mit FEAT_ASID16), `ASIDBits`: `0b0000`=8, `0b0010`=16 (D24-9247); `RKHXQX/INHTBK/IBZXYN` (D8-7855); bei `AS=0` werden TTBR-Bits [63:56] **als 0 behandelt** | nein |
| B6 | **Cortex-A72: `ASIDBits`?** (Plan: offen) | **bestätigt: 16 Bit** [gelesen] | TRM Table 4-35: `ASIDBits [7:4] = 0x2 „supports 16 ASID bits“` (S. 4-116); Resetwert `ID_AA64MMFR0_EL1 = 0x00001124` (Table 4-1, S. 4-77). Mit `AS=0` (Repo-Stand) nutzt der A72 nur 8 Bit; `MAX_PROCESSES=16` passt in 8 Bit. **Was QEMU `raspi4b`/`virt` melden: offen → 0b** | ja (Frage 15) |
| B7 | `nG=1` muss auf **allen** Anwendereinträgen stehen, sonst gelten sie für alle ASIDs | **bestätigt** [gelesen] | `RQGKGF`, `RXXNPZ` (D8-7853/7854). Zusatz: `TLBI ASIDE1` trifft nur **nicht-globale** Einträge der ASID (`RFKSVM`, D8-7868); globale Einträge überlebt es | ja (Frage 15) |
| B8 | Wechsel von TTBR0 samt ASID bei `A1=0`: Beispielfolge verlangt, dass TTBR1 währenddessen **nur globale** Einträge referenziert | **bestätigt, für NewTOS erfüllt** [gelesen] | Example D8-1 (D8-7700). Kernel-Einträge setzen `nG` nicht (`KA/mmu.rs:178-182,284`: kein `nG`-Bit), d. h. global ⇒ ein einziges `msr ttbr0_el1` + `isb` genügt | nein |
| B9 | Nach `msr ttbr0_el1` ist eine Kontext-Synchronisation nötig, bevor man sich auf die Wirkung verlässt; **`eret` ist beim A72 selbst eine**: Ausnahmeeintritt/-rückkehr sind CSEs, wenn FEAT_ExS fehlt | **bestätigt** [gelesen] | `IRBMTM` (D8-7700); D24.1.2.2 (D24-8470): *„Exception return, if FEAT_ExS is not implemented“*. FEAT_ExS ist ein späteres Merkmal; im A72-TRM sind `SCTLR_EL1` Bit 11 (EOS) und Bit 22 (EIS) „Reserved, RES1“ (Table 4-44, S. 4-126/4-127) ⇒ dauerhaft CSE. Repo schreibt `SCTLR_EL1` Init `0x30D0_0800` (Bits 11,22 gesetzt) | nein |
| B10 | `EPD0/EPD1` (Bits 7/23): `1` ⇒ Walks für diese Hälfte aus, Fehler = Translation-Fault **Level 0** | **bestätigt** [gelesen] | D24-9923, D24-9920/9921; `ICRKPZ` (D8-7848). **Wichtig:** wirkt nur bei TLB-*Miss* — bereits gecachte Einträge treffen weiter, EPD ersetzt also **kein** TLBI | ja (Hinweis in §3.2) |
| B11 | `TBI0=0` (Repo): Top-Byte zählt bei der Adressprüfung; Anwenderzeiger mit gesetzten Bits [63:48] ⇒ Translation-Fault L0 | **bestätigt** [gelesen] | `TBI0` D24-9917; `RVZZSZ` D8-7840 | nein |

---

## 3. TLB-Pflege

| ID | Aussage | Urteil | Beleg | Plan ändern |
|---|---|---|---|---|
| C1 | Einträge, die Translation-/Address-size-/Access-flag-Fault erzeugen, werden **nie** gecacht; gültige Einträge dürfen jederzeit (auch spekulativ) gecacht werden | **bestätigt** [gelesen] | `RXCLRD`, `RSQBCS`, `IRCGCH` (D8-7852); `RMLNTS/IXFTPJ` (D8-7840); `IWZCBG` (D8-7858) | nein |
| C2 | Plan §3.2: „Verengung (gültig→ungültig, RW→RO): TLBI nach dem Schreiben; Erweiterung (ungültig→gültig): keins“ | **korrigiert (Präzisierung)** [gelesen] | *ungültig→gültig*: kein TLBI nötig, aber **DSB** (Sichtbarkeit für den Walker, `RSPVBD` D8-7704) und **Kontext-Synchronisation** (ISB/`eret`) für Befehlsabrufe (`IWZCBG` D8-7858). *gültig→gültig mit größeren Rechten* (RO→RW, `UXN` 1→0): **TLBI nötig** — gültige Einträge sind gecacht (`RSQBCS`), und selbst ein **Permission-Fault-Eintrag darf gecacht sein** (`RJVXRH` D8-7841; `IWJLXV`: Software muss danach invalidieren). `RGPPYH` (D8-7858): *jede* Änderung eines Eintrags verlangt Invalidierung des zugehörigen TLB-Eintrags | ja (§3.2) |
| C3 | Break-before-make: „nur bei Änderung von Ausgabeadresse, Speichertyp oder Blockgröße nötig, **nicht** bei reinen Rechteänderungen“ | **korrigiert (teilweise richtig, unvollständig)** [gelesen] | `RWHZWS` (D8-7858/7859) verlangt BBM bei: (i) Änderung von Speichertyp/Shareability/Cacheability; (ii) Änderung der Ausgabeadresse, wenn alt oder neu Schreibrecht gewährt **oder** der Inhalt am neuen OA abweicht; (iii) Änderung der Blockgröße (Block↔Tabelle) ohne FEAT_BBML1/2; (iv) Anlegen eines globalen Eintrags, der nicht-globale überlagert. Reine `AP/UXN/PXN`-Änderungen im Blattdeskriptor stehen **nicht** in der Liste (⇒ kein BBM, aber TLBI, C2). **Zusätze:** Änderung von `APTable/UXNTable/PXNTable` verlangt BBM + TLBI aller Ebenen (`IYQWJF` D8-7757, `IZGSNV` D8-7759). FEAT_BBML1/2 gibt es auf dem A72 nicht **[Schluss]**: ARMv8.0 (TRM 1.2.1, S. 1-15), und das TRM listet kein `ID_AA64MMFR2_EL1` (Table 4-1 S. 4-75…4-77, Memory-Map S. 4-364), in dem das BBM-Feld lebt (ARM ARM D24-9258). Folge für Stufe 3 „Copy-on-differ“: Ersetzen eines 2-MiB-**Blocks** durch eine **Tabelle** (oder umgekehrt) = BBM; das Umhängen eines L2-Tabelleneintrags von geteiltem auf eigenes L3 ist in der Liste nicht ausdrücklich aufgeführt — **konservativ ebenfalls BBM** [Schluss] | ja (§3.2) |
| C4 | BBM-Folge | **bestätigt** [gelesen] | `RDDMVT` (D8-7859): (1) Eintrag ungültig, (2) DSB, (3) TLBI (Broadcast-Form), (4) DSB, (5) neuer Eintrag, (6) DSB | ja (aufnehmen) |
| C5 | TLBI-Varianten auf dem A72 | **bestätigt** [gelesen] | Unterstützt: `VMALLE1`, `VAE1`, `ASIDE1`, `VAAE1`, `VALE1`, `VAALE1` und jeweils `…IS` (TRM Table 4-6, S. 4-80). **Nicht** vorhanden: Bereichs-TLBI (`RVAE1…`), `…NXS` (kommen mit späteren FEATs; im TRM nicht gelistet). Alle TLBI sind **UNDEFINED an EL0** (`IDJQYS`, D8-7863) | ja (Aufnahme) |
| C6 | Umfang der Varianten | **bestätigt** [gelesen] | `VMALLE1`: alle Stage-1-Einträge des Regimes, alle ASIDs, auch globale (`RPMNQC` D8-7867/7868). `ASIDE1`: nicht-globale Einträge der ASID (`RFKSVM` D8-7868). `VAE1`: Einträge **jeder** Ebene für die VA — nicht-global mit passender ASID **und** globale der **letzten** Ebene (`RVMSWQ` D8-7868). `VAAE1`: alle ASIDs, jede Ebene (`RZSXRQ` D8-7869). **`…L`-Varianten (`VALE1`, `VAALE1`) betreffen nur die letzte Ebene** (`RDRCZZ`, `RJXKNN`) — **nach Änderung von Tabellen-/Zwischenebeneneinträgen (z. B. L2→anderes L3) unbrauchbar**, dort `VAE1/VAAE1/VMALLE1` | ja (Aufnahme) |
| C7 | Operand `Xt` | **neu** [gelesen] | Bits [63:48] = ASID (bei VA-Varianten ohne ASID RES0), [47:44] TTL-Hinweis (ohne FEAT_TTL RES0), [43:0] = `VA[55:12]` (`IZVNJG`, D8-7863/7864). Bei 8-Bit-ASID müssen die oberen 8 ASID-Bits 0 sein (`ILHKCP`, D8-7864) | ja (Aufnahme) |
| C8 | Barrieren: Schreiben der Tabelle → `DSB` → `TLBI` → `DSB` → `ISB` | **bestätigt** [gelesen] | Sichtbarkeit: `RSPVBD` (D8-7704) — *„only after the execution of a DSB“*; Abschluss: `IBLDZX` (D8-7875): *„complete after the execution of DSB by that PE, followed by a Context synchronization event“* (A72 ohne FEAT_ETS2 **[Schluss]**, ARMv8.0); `ITGVWD` (D8-7874): ohne DSB sind TLBI-Wirkungen relativ zu Ladungen/Speicherungen/Cache-Wartung in beliebiger Reihenfolge beobachtbar. Dass beim A72 für Tabellenänderungen **kein Cache-Clean** nötig ist: `ID_MMFR3.CohWalk = 0x1` (TRM Table 4-25, S. 4-102) — der Walker liest kohärent mit WB-Caches (Repo: TCR-Walk-Attribute WB/IS) | nein |
| C9 | TTBR0-Wechsel **ohne** ASID: Reihenfolge | **bestätigt** (Repo-Folge) / **Warnung** für den Plan [gelesen] | Repo `restore_boot_ttbr0` (`kernel/src/arch/aarch64/user_entry.rs:101-115` gelesen): `dsb ishst; msr ttbr0_el1, <leere L0>; tlbi vmalle1; dsb ish; isb`, danach Installation der neuen Wurzel. Das ist architekturgerecht: Gültige Einträge dürfen **jederzeit** (auch spekulativ) gecacht werden (`IRCGCH`), also darf TLBI **nicht** vor dem Umschalten bei noch gültiger alter Wurzel stehen und dann die neue Wurzel ohne weitere Pflege folgen; mit **leerer** Wurzel dazwischen kann nichts Gültiges nachgeladen werden (ungültige Einträge nie gecacht, `RXCLRD`). Gleichwertig und kürzer **[Schluss]**: direkter Wechsel A→B, dann `isb; tlbi vmalle1; dsb ish; isb` (solange der Kernel bis dahin keine Anwenderadresse über TTBR0 anfasst). Plan Stufe 1/3.16 behält die Leere-Wurzel-Folge: richtig | nein (Hinweis) |
| C10 | **TLBI unmittelbar vor `eret`** („nicht direkt vor `eret`“, Plan §3.2) | **korrigiert: keine Architekturregel** [gelesen] | Architektur: `TLBI; DSB ISH` schließen die Pflege ab, die Kontextsynchronisation liefert der `eret` selbst (B9, `IBLDZX` D8-7875). Die Plan-Regel ist ein **QEMU-TCG-Workaround** (QUIRK-001), kein ARM-Gebot → als solchen kennzeichnen; Spike 0b prüft, ob er auf QEMU 11.0.2 noch nötig ist | ja (§3.2) |
| C11 | `…IS`-Varianten bei nur einem aktiven Kern äquivalent | **bestätigt** [gelesen] | `…IS` wirkt auf alle TLBs der Inner-Shareable-Domäne (`IXMWNR` D8-7861, D8-7863; `RQRNJC` D8-7874); A72: `MaintBcst=0x2` „affect structures according to shareability“ (TRM Table 4-25, S. 4-102). Geparkte Kerne werden mit erfasst — harmlos **[Schluss]** | nein |
| C12 | A72-TLB-Aufbau | **neu** [gelesen] | L1-I-TLB 48 Einträge voll assoziativ, L1-D-TLB 32 voll assoziativ, L2-TLB 1024 Einträge 4-fach (TRM 1.3, S. 1-17; 5.2, S. 5-277); „TLBs … do not require to be flushed on a context or virtual machine switch“, kein Lockdown (S. 5-277); Treffer verlangt passende ASID **oder** global (S. 5-278) | nein (Info) |
| C13 | TLB-Konflikt (BBM verletzt) | **neu** [gelesen] | Mehrfachtreffer darf als *TLB conflict abort* gemeldet werden: `DFSC/IFSC = 0b110000` (`RWJMZJ`/`IJYDXJ` D8-7842; D24-8684). Muss der Fehlerklassifikator (Stufe 0c) als Kernelfehler behandeln, nicht als Anwenderfehler | ja (Frage 11) |

---

## 4. PAN, UAO, `LDTR`/`STTR`

| ID | Aussage | Urteil | Beleg | Plan ändern |
|---|---|---|---|---|
| D1 | Der A72 hat **kein PAN** | **bestätigt** [gelesen] | FEAT_PAN: *„OPTIONAL from Armv8.0 … mandatory from Armv8.1“* (ARM ARM A2.2, S. A2-101). A72 = ARMv8.0 (TRM 1.2.1, S. 1-15). Feld `ID_AA64MMFR1_EL1.PAN [23:20]` (D24-9253): `0b0000` ist erst ab 8.1 verboten. TRM: `ID_AA64MMFR1_EL1` steht **nicht** in Table 4-1 und die Debug-Memory-Map markiert `0xD58/0xD5C` als *„Memory Model Feature Register 1 … RES0“* (S. 4-364) ⇒ `PAN=0`. Zur Laufzeit gegenprüfen: `mrs ID_AA64MMFR1_EL1` (0b) | ja (Frage 10) |
| D2 | Folge: nichts verhindert in HW, dass EL1 versehentlich direkt eine Anwender-VA dereferenziert; `uaccess` muss **durch Disziplin** (nur `ldtr/sttr`) erzwungen werden | **bestätigt [Schluss]** | D1; wäre PAN vorhanden, würde es *privilegierte* Zugriffe auf EL0-zugängliche Adressen sperren (`RXTSQH`, D8-7775/7776) | ja (Frage 10) |
| D3 | UAO | **bestätigt: nicht vorhanden [Schluss]** | FEAT_UAO: optional ab 8.1, verpflichtend ab 8.2 (A2-112); Feld `ID_AA64MMFR2_EL1.UAO [7:4]` (D24-9262). Das A72-TRM führt `ID_AA64MMFR2_EL1` nirgends (Table 4-1, Memory-Map S. 4-364). Folge: `LDTR/STTR` sind an EL1 immer unprivilegiert | ja (Frage 10) |
| D4 | Semantik `LDTR/STTR` an EL1 | **bestätigt** [gelesen] | C6.2.245 (C6-2374): *„Explicit Memory effects … behave as if the instruction was executed at EL0 if the Effective value of PSTATE.UAO is 0 and … the instruction is executed at EL1“*. ⇒ geprüft werden die **EL0-Rechte** (`AP[1]=1`, Table D8-63; ob lesen/schreiben nach `AP[2]`); `UXN/PXN` sind für Datenzugriffe ohne Belang. `PSTATE.PAN` betrifft nur *privilegierte* Zugriffe (`RXTSQH`) ⇒ **unprivilegierte Zugriffe gehen auch mit PAN=1 durch** — `ldtr/sttr` ist also zugleich der PAN-feste Weg **[Schluss]** | ja (Frage 10) |
| D5 | Kernel-Adressen als Anwenderzeiger | **neu [Schluss]** | Kernel-Mappings haben `AP=00` (`KA/mmu.rs:83`) ⇒ Table D8-63: keine `Unpriv*`-Rechte ⇒ `ldtr` auf einen Kernelzeiger wirft *Permission-Fault*, ohne zusätzliche Bereichsprüfung. Adressen außerhalb TTBR0-Bereich (z. B. gesetzte Bits [63:48]) ⇒ Translation-Fault L0 (`RVZZSZ`) | ja (Frage 10) |
| D6 | Befehlsform | **neu** [gelesen] | `LDTR/STTR(+B/H/SB/SH/SW)`: Basisregister + **unskalierter signierter `imm9`** (−256…255), kein Writeback, kein Registeroffset (C6-2374); unprivilegierte Paar-/SIMD-Formen gibt es nur mit FEAT_LSUI (D8-7777) ⇒ Kopierschleifen sind 1–8 Byte je Befehl | ja (Hinweis) |
| D7 | Fehlermeldung eines `ldtr/sttr`-Aborts | **bestätigt** [gelesen] | Abort an EL1 ohne Ebenenwechsel ⇒ `EC=0b100101 (0x25)` „Data Abort taken without a change in Exception level“ (D24-8643); `FAR_EL1` = fehlerhafte VA (`RZTSSH` D1-7219; D24-8852); Art in `ISS.DFSC`, Schreiben/Lesen in `ISS.WnR` (D24-8682/8683). EL0-Abbruch dagegen `EC=0x24`. Ein EL1-Abbruch auf Anwenderzeiger ist von einem Kernelfehler nur über den **PC** (Fixup-Tabelle der `uaccess`-Befehle) unterscheidbar — die Architektur liefert dazu nichts **[Schluss]** | ja (Frage 10/11) |

---

## 5. Caches, EL0-Cache-Pflege, Code laden

| ID | Aussage | Urteil | Beleg | Plan ändern |
|---|---|---|---|---|
| E1 | `SCTLR_EL1.UCI` (Bit 26): `0` ⇒ `DC CVAU/CIVAC/CVAC` und `IC IVAU` an EL0 trappen nach EL1 mit **EC `0x18`**; `UCT` (Bit 15) analog für `CTR_EL0`; `DZE` (Bit 14) für `DC ZVA` (bei `0` liest EL0 `DCZID_EL0.DZP=1`) | **bestätigt** [gelesen] | ARM ARM D24-9704 (UCI), D24-9710 (UCT, DZE); TRM Table 4-44 (S. 4-126/4-127: dort UCI nennt `DC CVAU, DC CIVAC, DC CVAC, IC IVAU`). Repo-Init lässt alle drei aus (`boot.rs:189-198`) | ja (E13) |
| E2 | EL0 braucht für diese Befehle **Leserecht** auf die Adresse | **bestätigt** [gelesen] | D7.5.9.3 (D7-7659): Permission-Fault möglich, wenn EL0 die Adresse nicht lesen kann (für `IC IVAU` implementierungsdefiniert). Schreibrecht erst mit FEAT_CMOW (nicht A72 **[Schluss]**, ARMv8.0). Fehler: `EC 0x24`, `ISS.CM=1`, `WnR=1` (D24-8682/8683) | ja (E13) |
| E3 | Code nach dem Schreiben ausführbar machen: `DC CVAU; DSB ISH; IC IVAU; DSB ISH; ISB` | **bestätigt** [gelesen] | B2.2.5 (S. B2-253) wörtlich dieselbe Folge; Repo `sync_code_cache` (`NT/mmu.rs:445-489`, laut Plan) entspricht. Erforderlich, weil der A72 `CTR_EL0.IDC/DIC` nicht hat (Bits [30:28] RES0, TRM Table 4-40, S. 4-122; Architektur: Wartung entfällt nur bei `DIC=1`, `RWNDVG` D8-7876) | nein |
| E4 | L1-I-Cache ist PIPT ⇒ Wartung über **irgendeinen** Alias genügt (Kernel-Alias) | **bestätigt (A72-spezifisch)** [gelesen] | TRM: `CTR_EL0.L1Ip = 0b11` PIPT (S. 4-122), 6.1 (S. 6-285: I- und D-Cache PIPT). Architektur: PIPT-Wartung wirkt auf alle VA-Aliase (`RYXNGL` D8-7876); bei VIPT nur für die angegebene VA (`RLYZYY` D8-7877) ⇒ **Kernelinterne Eigenschaft des A72, nicht Vertrag** | nein (als „intern“ vermerkt) |
| E5 | Geometrie | **neu** [gelesen] | Zeilenlänge **64 Byte** (`CTR_EL0.DminLine=IminLine=0x4` = 16 Worte, CWG=ERG=16 Worte, S. 4-122; `CCSIDR.LineSize=0b010`, S. 4-118); L1-I 48 KiB 3-fach, L1-D 32 KiB 2-fach (S. 1-17, 6-285); L2 512 KiB/1/2/4 MiB, 16-fach konfigurierbar (S. 1-17, 4-118). **Welche L2-Größe der BCM2711 hat, steht nicht im TRM** → zur Laufzeit `CCSIDR_EL1` lesen | nein (Info) |
| E6 | Aliasing Kernel/Anwender: Mismatched-Attribute-Regel gilt nur bei Typ/Shareability/Cacheability | **bestätigt** [gelesen] | B2.11 (B2-331/332), D8.18.3 (D8-7877). Plan §3.1/ADR-023: einzige Ausnahme Bildschirm (Normal-NC) mit Clean+Invalidate | nein |

---

## 6. Abort-Klassifikation (`ESR_EL1`, `FAR_EL1`)

| ID | Aussage | Urteil | Beleg | Plan ändern |
|---|---|---|---|---|
| F1 | EC-Werte: Instruction Abort aus niedrigerer EL `0x20`, ohne Wechsel `0x21`; Data Abort `0x24`/`0x25` | **bestätigt** [gelesen] | ESR_EL1 EC-Tabelle D24-8643. Repo-Konstanten `KA/exceptions.rs:88-95` (gelesen) stimmen | nein |
| F2 | Fehlertyp aus `DFSC`/`IFSC` (Bits [5:0]) | **bestätigt, Plan-Angabe präzisiert** [gelesen] | Address-size `0b0000LL`; **Translation `0b0001LL`** (L0…L3 = `0x04…0x07`); Access-flag `0b0010LL` (`0x09…0x0B`, `0x08` nur mit LPA2); **Permission `0b0011LL`** (`0x0D…0x0F`, `0x0C` nur mit LPA2); synchroner External Abort `0b010000`; Alignment (nur DFSC) `0b100001`; TLB-Konflikt `0b110000` (D24-8672/8673 IFSC, D24-8683…8685 DFSC). QUIRK-003: `IFSC=0x0e` = Permission-Fault Level 2 ✓ | ja (Frage 11) |
| F3 | Ebene des Fehlers | **bestätigt** [gelesen] | Translation: Ebene des fehlenden Eintrags; **Permission: Ebene des Block-/Seitendeskriptors**, auch wenn `APTable` die Ursache ist; außerhalb des Bereichs bzw. bei `EPDn`: Level 0 (`ICRKPZ` D8-7848; `RCSRQR` D8-7841) | nein |
| F4 | `WnR` (Bit 6, nur Data Abort) | **bestätigt** [gelesen] | `1` = Schreibzugriff verursachte den Abort; bei Cache-Wartung/AT-Fehlern immer `1` (D24-8682/8683). Bei Instruction Aborts kein `WnR` (Bit 6 RES0, D24-8672) | ja (Frage 11) |
| F5 | `FAR_EL1`-Gültigkeit | **bestätigt** [gelesen] | Für alle Instruction-/Data-Aborts wird die fehlerhafte VA nach `FAR_ELx` geschrieben (`RZTSSH` D1-7219; D24-8852). `FnV` gilt **nur** bei synchronem External Abort (`DFSC/IFSC=0b010000`; sonst RES0) (D24-8681/8672). ⇒ Für MMU-Fehler ist `FAR_EL1` immer gültig | ja (Frage 11) |
| F6 | **Permission- vs. Translation-Fault für `PROT_P/S/I` Nicht-Besitzer** | **neu [Schluss]** | Plan §3.3 macht Nicht-Besitzereinträge **ungültig** (Bit 0 = 0) ⇒ der Zugriff meldet *Translation Fault* (`RDYNYQ` D8-7840), **nicht** Permission Fault. Der Klassifikator kann „geschützt“ von „nicht abgebildet“ nur über die **Regionstabelle** (FAR) unterscheiden, nicht aus `ESR` — der Plan sieht das ohnehin vor (Frage 11); hier die Begründung. Nur `PROT_PR` (Nicht-Besitzer `AP=11`, Schreibversuch) liefert einen echten Permission-Fault (`WnR=1`) | ja (Frage 11) |
| F7 | EL0- vs. EL1-Fehler auf Anwenderzeiger | **bestätigt** [gelesen] | `EC 0x24` (EL0) vs. `0x25` (EL1, z. B. `ldtr`); siehe D7 | nein |
| F8 | Prioritäten: Translation (je Ebene) vor Access-flag vor Permission | **neu** [gelesen] | D8.15.5 (D8-7849/7850): Reihenfolge je Zugriff; Permission-Fault erst, wenn die Übersetzung gelang | nein (Info) |

---

## 7. Cortex-A72-Besonderheiten, die den Plan einschränken

| ID | Befund | Urteil | Beleg | Plan ändern |
|---|---|---|---|---|
| G1 | A72 = **ARMv8.0-A**; keine PAN, UAO, VHE, HPDS, BBM, CnP, HW-AF/Dirty, ExS, ETS2, CMOW, TTL, Range-TLBI, BTI/MTE/PAuth **[Schluss]** aus: Table 4-1 enthält nur `ID_AA64PFR0/DFR0/ISAR0/MMFR0`; `PFR1/DFR1/ISAR1/MMFR1` stehen in der Memory-Map als `RES0` (S. 4-364); `ID_AA64MMFR2_EL1` fehlt ganz. **Gelesen** ist nur die Registerliste; „Merkmal X fehlt“ ist der Schluss daraus | **bestätigt (mit Laufzeitprüfung)** | TRM S. 4-75…4-77, 4-364, 1-15 | ja |
| G2 | `ID_AA64MMFR0_EL1 = 0x00001124`: PARange 44 Bit, ASIDBits 16, BigEnd mixed, SNSMem, **4 KiB und 64 KiB Granule ja, 16 KiB Granule nein** | **bestätigt** [gelesen] | Table 4-35 (S. 4-116) | nein |
| G3 | `SCTLR_EL1` des A72: `SA0` (EL0-Stackausrichtung) hat Resetwert **1** — im Repo-Init (`0x30D0_0800`) ist Bit 4 = 0 | **Info [gelesen]** | TRM Table 4-44 (S. 4-128) | nein |
| G4 | **Errata** zu TLBI/ASID | **offen** | Das TRM enthält keine Errata-Liste, nur „Errata fixes“ in der Revisionshistorie (1.8, S. 1-23). Das Software-Developer-Errata-Notice ist **nicht** gepinnt ⇒ eine Aussage „keine bekannten TLBI/ASID-Errata“ wäre erfunden | nein (→ §10) |
| G5 | `DCZID_EL0 = 0x4` (BS=4: 64-Byte-Block, DZP=0); `DC ZVA` an EL0 nur mit `DZE=1` | **bestätigt** [gelesen] | TRM Table 4-1 (S. 4-77), 4.3.27 (S. 4-122/4-123); ARM ARM D24-9710, D24-8632 | nein |

---

## 8. AAELF64

| ID | Aussage | Urteil | Beleg | Plan ändern |
|---|---|---|---|---|
| H1 | `R_AARCH64_RELATIVE` = **1027** (ELF64), Operation `Delta(S) + A`, RELA | **bestätigt** [gelesen] | AAELF64 5.7.15 „Dynamic relocations“, Tabelle S. 40 (`1027 / 183 R_<CLS>_RELATIVE Delta(S)+A`) | ja (Zahl belegt) |
| H2 | Berechnung `*(u64*)(B + r_offset) = B + r_addend`, `B` = Ladebasis | **bestätigt (über die Prosa)** [gelesen] | S. 41: RELATIVE ist *„a relative adjustment to the place based on the load address of the object relative to its original link address … If S is the null symbol … the adjustment is based on the segment defining the place“*. Die formale Definition von `Delta(S)` (S. 20) ist im Vorzeichen missverständlich formuliert („difference between the static link address … and the execution address“); maßgeblich für das Vorzeichen ist die Prosa von S. 41 ⇒ Wert = Addend + (Lade- − Linkadresse). Das Ziel ist ein **8-Byte-ausgerichteter 64-Bit-Platz** (S. 40) | ja |
| H3 | Dynamische Relokationstypen, die es überhaupt gibt (ELF64) | **bestätigt** [gelesen] | `ABS64`(257), `COPY`(1024), `GLOB_DAT`(1025), `JUMP_SLOT`(1026), `RELATIVE`(1027), TLS-Typen (1028–1031), `IRELATIVE`(1032), `AUTH_*` (S. 40). `COPY` darf nur in `ET_EXEC` stehen (S. 41) ⇒ in `ET_DYN` ausgeschlossen. RELATIVE ersetzt `GLOB_DAT`, wenn das Symbol zur Linkzeit sicher in der eigenen Einheit auflöst (S. 41) | nein |
| H4 | „Nur `R_AARCH64_RELATIVE`“ reicht für ein `-fPIE -pie` statisches Image | **offen (Schluss plausibel, nicht aus der Spezifikation belegbar)** | Nach H3 braucht ein symbolloses Image nur `RELATIVE` **[Schluss]**; ob der Linker stattdessen `ABS64` gegen lokale Symbole oder `IRELATIVE` (ifunc) ausgibt, ist **Werkzeugverhalten**, nicht Spezifikation ⇒ Toolchain-Spike Stufe 0d (`llvm-readelf -r`). Plan-Politik „alles andere → `EPLFMT`“ bleibt zulässig | ja (Hinweis) |
| H5 | `Elf64_Rela`-Aufbau (`r_offset`, `r_info`, `r_addend`), `ELF64_R_TYPE(r_info)` | **offen (nicht in AAELF64)** | AAELF64 sagt nur, der Typcode stehe in der `ELF64_R_TYPE`-Komponente von `r_info` (5.7.3, S. 19), RELA sei zu nutzen, wenn der Addend nicht in den Platz passt (S. 19) und RELA sei idempotent (5.7.19, S. 42). Die Strukturdefinition liegt im SCO/gABI-ELF-Standard (S. 8: `SCO-E`) — **nicht gepinnt** | nein (→ §10) |
| H6 | `ET_DYN`, `PT_DYNAMIC`, `DT_RELA/RELASZ/RELAENT/RELACOUNT`, `DT_TEXTREL`, `PT_INTERP` | **offen (nicht in AAELF64)** | AAELF64 6.3–6.5 (S. 43/44): *„The information on program loading / dynamic linking / the Dynamic Section has moved to [SYSVABI64]“*. Diese Quelle ist nicht gepinnt. `DT_RELACOUNT` kommt in **keiner** der drei gepinnten Quellen vor (kein Beleg, ob es zur Spezifikation oder zu einer Linkererweiterung gehört) ⇒ Lader darf sich **nicht** darauf stützen | nein (→ §10) |
| H7 | ELF-Kopf-Prüfungen für den Lader | **neu** [gelesen] | `e_machine = EM_AARCH64 (183, 0xB7)`; `e_flags = 0` („no processor-specific flags“); `EI_CLASS = ELFCLASS64`; `EI_DATA` Little-Endian für unsere Umgebung; `EI_OSABI = 0`, solange keine OS-spezifischen Flags (5.2, S. 13) | ja (Lader §3.5) |
| H8 | Programmkopf-Besonderheiten AArch64 | **Info [gelesen]** | `PT_AARCH64_ARCHEXT/UNWIND/MEMTAG_MTE` reserviert, keine AArch64-`p_flags` (6.1, S. 43) | nein |

---

## 9. QUIRK-001 und ADR-010 „Wand 3“ — Architektur gegen QEMU

(Für den QEMU-Teil ist Stufe 0b zuständig; hier nur: *Was davon ist eine Architekturregel, was nicht?*)

| ID | Aussage in QUIRK-001 / ADR-010 | Einordnung | Beleg |
|---|---|---|---|
| I1 | Gleiche VA, andere PA, gleiche ASID ⇒ beim Wechsel muss der TLB gereinigt werden | **Architekturregel** (richtig). Globale/ASID-gleiche Einträge werden nicht von selbst unterschieden (`RXXNPZ` D8-7853, `RGPPYH` D8-7858); Zwischenebenen-Einträge sind stets ASID-getaggt (`RCHVGJ`) | siehe C1–C9 |
| I2 | Die Repo-Folge `msr ttbr0 (leer); tlbi vmalle1; dsb ish; isb` + Installation | **architekturvollständig** (C9) | `user_entry.rs:101-115` |
| I3 | Eine Zeile **UART-Schreibverkehr** zwischen TLBI und Installation ist nötig | **kein Architekturgebot — reiner QEMU-Workaround.** Die Architektur kennt als Abschluss nur `DSB` + Kontextsynchronisation (`IBLDZX` D8-7875); „Device-Writes beenden den TCG-Block“ steht nirgends | — |
| I4 | „Auf echter HW kann das nicht passieren, weil der L1-I-Cache PIPT ist“ | **Teilweise falsch begründet (korrigiert).** PIPT stimmt für den A72 (TRM S. 4-122, 6-285), schützt aber nur den *Cache*; die Aliasfreiheit der **TLB-Einträge** beruht auf dem TLBI (I1). Auf echter HW ist die Reihenfolge nicht „unerheblich“, sondern durch DSB/TLBI/ISB definiert | D8-7876, D8-7852 |
| I5 | Ein **ASID-getaggtes** `msr ttbr0_el1` hängt QEMU | **QEMU-only.** Architektur: der Schreibvorgang ist zulässig (ASID in [63:48], `A1=0`, A72 16 Bit: B4–B6) | D24-10041/10042; TRM S. 4-116 |
| I6 | „Wand 3“: Kernel-Alias (TTBR1, EL1-Normal) **und** Anwender-Alias (TTBR0, EL0-Normal, PXN) derselben DRAM-Seite hängt den ersten Kernel-Store | **QEMU-only (bzw. nicht aus der Architektur ableitbar).** Aliase mit unterschiedlichen **Rechten**, aber gleichem Typ/Shareability/Cacheability sind nach B2.11 ausdrücklich erlaubt; „mismatched“ wären nur abweichende Typ-/Shareability-/Cacheability-Attribute. ADR-010 selbst vermerkt „Real ARMv8 permits this“ — bestätigt | B2.11 (B2-331/332) |
| I7 | `tlbi vmalle1` auf dem **Install**-Pfad hängt QEMU früh | **QEMU-only.** Architektur: TLBI nach TTBR0-Installation ist zulässig und (wegen C9) sogar die sichere Folge | — |
| I8 | „Ein einziges, nie getauschtes TTBR0 ⇒ ASIDs unnötig, kein TLBI beim Wechsel, QUIRK-001 entfällt“ | **Architekturrichtig** (kein Wechsel ⇒ nichts zu invalidieren). Im Plan (eine Wurzel je `memspace`) gilt es nur noch für Prozesse im selben `memspace` | — |
| I9 | ADR-010 „Neutral: User-RWX, kein W^X“ | **Architektur:** `AP=01 + UXN=0` ⇒ EL0 darf ausführen; EL1-Ausführung ist durch `RVWLLR` implizit entfernt; globales W^X wäre `SCTLR_EL1.WXN` (TRM S. 4-126) | D8-7758, D8-7759 |
| I10 | ADR-010-Stage-0-Befund „Kernel-Heap greift über Low-Half-Identität auf PA zu“ | **n/a** (Software-Befund, erledigt) | — |

**Zusammenfassung:** Von den drei ADR-010-„Wänden“ ist **keine** eine Architekturregel. Wand 1 (TTBR0-Wechsel) hat einen architektonischen Kern (TLBI), den die Repo-Folge erfüllt; der UART-Trick ist QEMU. Wand 2 (ASID) und Wand 3 (Aliasing) sind nach den Quellen architekturell zulässig ⇒ QEMU-Verhalten ⇒ Stufe 0b.

---

## 10. Was der Plan ändern muss (Kurzliste)

Alle Punkte sind in `arch03-implementation-plan.md` mit `0a:` eingearbeitet.

1. **Quellenlage / Legende:** ARM ARM, A72-TRM, AAELF64 liegen jetzt in `references/arm-arm/`; Ausgabe ist **M.d** (nicht M.c). Stufe 0a ist erledigt, **Rest-„offen“** wandert nach 0b/0d (siehe §11).
2. **§3.2 Änderungsregel** (C2, C3, C4, C10): TLBI auch bei *Erweiterung* gültiger Einträge; BBM-Liste vollständig (Typ/Shareability/Cacheability, OA mit Schreibrecht/Inhaltsabweichung, Block↔Tabelle, Tabellenattribute); konservativ BBM auch beim Umhängen L2→privates L3; BBM-Folge `RDDMVT`; „kein TLBI vor `eret`“ als **QEMU-Workaround** kennzeichnen (Architektur: `TLBI; DSB ISH` + `eret` als CSE).
3. **§3.2 Invariante** (A8): Bits [63:59] aller Tabellendeskriptoren = 0 (auf dem A72 nicht abschaltbar). **Konsistenzhinweis** (kein Track-B-Fund): „sein L2-Eintrag zeigt auf eigenes L3“ setzt ein **eigenes L2** für jedes GiB voraus, in dem ein `memspace` abweicht — für Pi (Pool im ersten GiB, L2₀ ohnehin je `memspace`) erfüllt, für `qemu-virt` (Pool ab `0x4000_0000`, zweites GiB, geteiltes L2) **nicht**.
4. **§3.3** (A3, A6): `AF=1` zwingend (kein HW-AF); PXN-Begründung (wirkt real nur bei `AP=11`); Bitlagen sind jetzt Primärquelle.
5. **Frage 10/§3.6** (D1–D7): PAN fehlt, UAO fehlt; `ldtr/sttr`-Semantik, `imm9`-Form, Kernelzeiger fallen automatisch durch (`AP=00`); Fehlerbild `EC 0x25`, Unterscheidung nur per PC.
6. **Frage 11** (F2–F6, C13): DFSC/IFSC-Kodierung bestätigt; `FnV` nur bei External Abort; **`PROT_P/S/I`-Nicht-Besitzer erzeugen Translation-Faults** (Klassifikation über Regionstabelle); TLB-Konflikt `0b110000` = Kernelfehler.
7. **Frage 15 / §3.16 / Stufe 4** (B4–B8, C5–C7, C11): A72 hat **16-Bit-ASIDs** (`ASIDBits=0x2`), `AS=0` reicht (8 Bit ≥ 16 Prozesse); `nG=1` auf allen Anwendereinträgen; `TLBI ASIDE1` lässt globale Einträge stehen; `Xt`-Format; bei ASID-Rollover `VMALLE1(IS)` statt nur `ASIDE1`; `L`-Varianten nach Tabellenänderung unbrauchbar.
8. **Frage 3 / §3.5** (H1–H7): `R_AARCH64_RELATIVE = 1027`, Berechnung belegt; ELF-Kopf-Prüfungen (`EM_AARCH64`, `e_flags=0`, ELFCLASS64, LE); **`Elf64_Rela`-Aufbau und `DT_*` bleiben offen** (nicht in AAELF64) — Lader darf `DT_RELACOUNT` nicht voraussetzen.
9. **E13** (E1, E2): `UCI`-Bedeutung belegt (EC `0x18` bei `0`; EL0 braucht Leserecht); kein Handlungsbedarf, solange der Kernel beim Laden synchronisiert.
10. **Frage 1/2, QUIRK-001, ADR-010** (I1–I10): Einordnung Architektur vs. QEMU; „PIPT ⇒ Reihenfolge unerheblich“ ist falsch begründet.

---

## 11. Was die Quellen nicht entscheiden (→ Stufe 0b bzw. weitere Quelle)

**Für Spike 0b (QEMU/Pi messen):**

1. Meldungen der Identifikationsregister unter QEMU `raspi4b` **und** `virt` und auf der Pi: `ID_AA64MMFR0_EL1.ASIDBits`, `ID_AA64MMFR1_EL1` (PAN, HPDS, HAFDBS), `ID_AA64MMFR2_EL1` (UAO, BBM), `CTR_EL0` (IDC/DIC), `CCSIDR_EL1` (L2-Größe der Pi). Die A72-Werte stehen im TRM; was QEMU simuliert, nicht.
2. Ob die vier QEMU-Workarounds noch nötig sind (UART-Klammer, ASID-getaggtes TTBR0, Kernel/Anwender-Aliasing, `TLBI` direkt vor `eret`) auf QEMU 11.0.2.
3. Ob die Pi das „`TLBI; DSB ISH; eret`“ der Architektur (C10) ohne Klammer verträgt.
4. Leistung 2-MiB-Blöcke vs. 4-KiB-Seiten (A72: L1-TLB nativ 4 KiB/64 KiB/1 MiB, A9).

**Weitere Quellen nötig:**

5. **ELF-gABI / SYSVABI64** (Elf64_Rela, `ELF64_R_TYPE`, `PT_DYNAMIC`, `DT_RELA/RELASZ/RELAENT`, `DT_TEXTREL`, `ET_DYN`-Anforderungen) — AAELF64 verweist darauf (S. 43/44); Vorschlag: in `references/README.md` pinnen. `DT_RELACOUNT` ist in keiner gepinnten Quelle belegt.
6. **Cortex-A72 Software Developer Errata Notice** (TLBI/ASID-Errata) — nicht gepinnt (G4).
7. **Linkerverhalten** (nur `RELATIVE`? `IRELATIVE`? `-z text`?) — Toolchain-Spike 0d, kein Dokument.

---

## 12. Beobachtungen außerhalb des Auftrags (je eine Zeile)

- Kernel-DRAM-Abbildung hat `PXN` nicht gesetzt (`KA/mmu.rs:179,284`), der Kernel-Alias von Anwenderseiten ist damit für EL1 ausführbar; „der Kernel führt nie Anwendercode über den Alias aus“ (Plan Frage 2) ist Disziplin, nicht Hardware-Garantie — Härtung wäre ein eigener Punkt.
- Plan §3.2 vs. Stufe 3 (geteiltes L2 im zweiten GiB) — siehe §10 Punkt 3.
- `docs/MEMORY.md` QUIRK-001 „Key insight“ und ADR-010 „Real ARMv8 … L1 I-caches are PIPT and never alias“: Begründung gemäß I4 anzupassen — **nicht von mir geändert** (Auftrag: nur Plan + dieses Dokument).
