# BIOS-Audit — Findings-Katalog

> Stand: 2026-07-23. NewTOS-Ist vs. EmuTOS-Quelle + Spec (`docs/api/bios/`) +
> COMPATIBILITY-Contract. Übersicht: `00-DASHBOARD.md`. Scope: BIOS Trap #13
> (12 Fn) + Boot-Init/Sysvar/OSHEADER/Cookie-Jar. XBIOS = BB4. HW-Backend-
> Adaption (EMMC-SD statt WD1772/ACSI, USB-HID statt IKBD-ACIA, kein MFP/Shifter)
> = dokumentierte Deviation, kein Bug.

## BLOCKER

### B1 — Low-Memory-Sysvars + OSHEADER + Cookie-Jar für native EL0-Apps nicht angelegt
- **EmuTOS:** `bios/bios.c:356-373` (`sysbase = &os_header;` @358), `bios/startup.S:73-114` (`_os_header`), `tosvars.ld:22-135` (Sysvar-Adressen), `util/cookie.c:26-35` (`cookie_init` publiziert `p_cookies` @`$5A0`), `bios/machine.c:665` (`fill_cookie_jar`). Alles läuft in `bios_init()`/`biosmain()` vor jedem Programmstart (`_boot-init.md` §1–§4).
- **NewTOS:** Grep über `kernel/src`+`tos-api/src` → 0 Instanzen von sysbase/os_header/OSHEADER/`$4F2`/`$5A0`-Befüllung. `kernel/src/bios/machine/layout.rs:305-313`: ST-RAM-Pool ab `img_end`; kein reservierter `$400`–`$5FF`-Bereich, keine OSHEADER-Instanz. Cookie-Jar kernel-owned ohne `$5A0` (ADR-012).
- **Failure (CONFIRMED, `sysvar-osheader-lowmem-gap.md`):** TeraDesk faultet in EL0 bei `main.c:56` `*(long*)0x4F2` → `ldrh` auf Müll-Zeiger, Data Abort ELR=0x11814. Jede 1:1-portierte TOS-App mit klassischem Versions-Check trifft dieselbe Klasse.
- **B1a — Cookie-Jar via Setexc doppelt blockiert:** `Setexc(0x5A0/4,-1)` = `Setexc(0x168,-1)` (TeraDesk `cookie.c:28,56`). NewTOS `NVEC=0x120` (`bios/vectors.rs:21`); `setexc` gibt für `num>=NVEC` 0 (`vectors.rs:74`). `0x168>=0x120` → immer 0.
- **B1b — os_version nicht gemeldet:** ohne OSHEADER kein `os_version`. GEMDOS meldet `Sversion`=0x3000, BIOS meldet keine TOS-Version. (S3.)
- **Bug-vs-Deviation:** echte Divergenz (beobachtbar via `$4F2`/`Setexc($5A0/4,-1)`); reine Software-Struktur, kein HW-Thema.
- **Soll-Fix: OFFEN** (Lead-Machbarkeits-/Design-Entscheidung: ADR-012/„Zwei-Welten" umkehren + Low-Memory je Prozess in EL0-VA + OSHEADER + `$5A0`-Sichtbarkeit **vs.** App-Patch).

## HIGH

### H1 — etv_*-Hooks per Setexc gespeichert, aber nie dispatcht
- **EmuTOS:** Boot-Defaults callable `bios/bios.c:360-362` (`etv_timer=just_rts`, `etv_critic=default_etv_critic`, `etv_term=just_rts`). Dispatch: etv_timer je 200-Hz-Tick, etv_critic bei Disk-Fehler (`blkdev.c:520-521`), etv_term bei Pterm.
- **NewTOS:** `bios/vectors.rs` — get/set/-1-Query/return-old ✓, aber Reader `vector(num)` ist `#[allow(dead_code)]` (`vectors.rs:87-94`), kein Aufrufer. Boot-Defaults sind nicht-callable Sentinels (`ETV_DEFAULT_JUST_RTS=0x4E75...`, `ETV_DEFAULT_CRITIC=0x4372...`, `vectors.rs:45,54`) → „chain to old" crasht.
- **Failure (CONFIRMED, statisch):** `Setexc(0x102,handler)` Terminate-Hook feuert nie; `Setexc(0x100,...)` Timer feuert nie; `Setexc(0x101,...)` eigener Critical-Handler feuert nie.
- **Soll-Fix:** etv_term im Pterm-Pfad, etv_critic im Rwabs-Fehlerpfad (M1), etv_timer im Tick-Pfad aufrufen; Boot-Defaults auf echt-callable No-op/Passthrough.

## MEDIUM

### M1 — Rwabs ruft bei Disk-Fehler keinen etv_critic-Pfad auf
- **EmuTOS:** `blkdev.c:520-521` — nach erschöpften Retries + logischem Request: `call_etv_critic(retval,dev)`; `CRITIC_RETRY_REQUEST`(0x00010000) → äußere Retry-Schleife. Verbindet zu AES `eralert`/Abort-Retry (`04-rwabs.md` §9).
- **NewTOS:** `bios/mod.rs:136-149` — bei `!ok` direkt EWRITF/EREADF; kein etv_critic, keine Retry-Kette.
- **Failure (PLAUSIBLE):** Programm mit eigenem etv_critic sieht bei SD-Fehler nie seinen Handler; roher EREADF/EWRITF. Überlappt H1 + AES-eralert-Deferral P4-012. Fehlercode-Kontrakt selbst korrekt.

### M2 — Getbpb: `reserved==0` nicht auf 1 normalisiert
- **EmuTOS:** `blkdev.c:702-705` — `if (reserved==0) reserved=1;` dann `fatrec=reserved`.
- **NewTOS:** `bios/bpb.rs:63,86-87` — `reserved` roh; `fatrec=reserved+(num_fats-1)*fsiz`, `datrec=...`. Bei `reserved==0` liegen fatrec/datrec eine FAT-Region zu tief.
- **Relevanz:** die BPB speist echte FS-Ops (`bdos/fs/fat.rs:61` `bpb::geometry` → `fat.rs:180,204,286,307,347`). Falsche fatrec/datrec → falsche Sektoren → FS-Korruption (Risikoklasse wie der historische Frename-FAT-Bug).
- **Failure (PLAUSIBLE):** malformierter Boot-Sektor reserved=0. Normale 500-MB-FAT16 = reserved=1 → nicht im Normalbetrieb.

### M3 — Getbpb: FAT16-`>65524`-Reject fehlt + verfrühte u16-Truncation
- **EmuTOS:** `blkdev.c:730-743` — `tmp` ULONG; `if (tmp > MAX_FAT16_CLUSTERS(65524) || fsiz==0) { recsiz=0; return 0; }`.
- **NewTOS:** `bpb.rs:71` rejectet FAT32 nur via `fsiz16==0`; `bpb.rs:88-92` `numcl=((total-datrec)/clsiz) as u16` — Truncation VOR Obergrenzen-Prüfung. >65535 wrappt (70000→4464), `>65524`-Reject fehlt.
- **Failure (PLAUSIBLE):** Disk >65524 Cluster + `spf16!=0` → EmuTOS lehnt ab, NewTOS mountet mit falschem numcl. Reale 500-MB-Partitionen <65524 → nicht im Normalbetrieb.

### M4 — Getbpb: fehlende Validitäts-Guards (FAT-Count, recsiz, clsizb-Overflow)
- **EmuTOS:** `blkdev.c:642-670` — `if (recsiz > max_sect_siz) return 0;` · `clsizb=spc*recsiz; if (clsizb==0 || clsizb > 32768) return 0;` · `if (b->fat==0) b->fat=2;` · `if (b->fat<1 || b->fat>2) return 0;`.
- **NewTOS:** `bpb.rs:71,83` — `clsizb=recsiz.wrapping_mul(clsiz)` (wrapping, keine `>32768`-Prüfung, keine recsiz-Obergrenze). FAT-Count: `num_fats==0→None` (EmuTOS normalisiert auf 2 + mountet); `num_fats>2` akzeptiert (EmuTOS rejectet).
- **Failure (PLAUSIBLE):** (a) FAT-Count-Feld 0: EmuTOS mountet, NewTOS gibt NULL → Laufwerk unzugänglich. (b) hostile recsiz/spc → überlaufende clsizb.

## LOW

### L1 — Rwabs: physical-mode (RW_NOTRANSLATE, Bit 3) nicht implementiert
`blkdev.c:438,483-504` (logisch vs. physisch je Bit 3). NewTOS `bios/mod.rs:108-129` immer logisch (Partitions-Tabelle), Bit 3 ignoriert. Dokumentiert `04-rwabs.md:432`. Selten genutzt.

### L2 — Bconin: conterm-Bit3-Shift-Fold fehlt
`bios.c:948-956` foldet shifty in Bits 24-31 wenn `conterm&8`. Default `conterm=7` → kein Fold. NewTOS `chardev.rs:60-64` foldet nie. Da conterm kein Sysvar (B1) und Default keinen Fold macht → Default-Verhalten identisch; Edge.

### L3 — Getbpb: fatrec/datrec bei num_fats≥3
`blkdev.c:711-713` nutzt bei ≥2 FATs die 2. FAT (`fatrec=reserved+fsiz`). NewTOS `bpb.rs:86-87` die letzte (`reserved+(num_fats-1)*fsiz`). Bei 1/2 FATs identisch; Divergenz erst ab 3 (EmuTOS rejectet 3 ohnehin, M4). Vollständigkeit.

## SPEC/DOC-Korrekturen (kein Code-Bug)

### S1 — `docs/api/bios/09-mediach.md` „Pi binding" stale
Spec (235-236, 261-265) behauptet Mediach „returns 0 unconditionally, ignores drv". Code ist bereits korrekt: `bios/mod.rs:159-165` gibt EUNDEV für invalid/unmounted, 0 für valide — matcht `blkdev.c:789-790`. Spec-Text veraltet, nicht der Code.

### S2 — COMPATIBILITY.md §System-variables + ADR-012 durch ADR-019/TeraDesk widerlegt
COMPATIBILITY.md:76-93 („Zwei-Welten": native→API, m68k→Low-Memory) + ADR-012 (Cookie-Jar kernel-owned, kein `$5A0`) beruhen auf „native Ports lesen Sysvars nur via API". TeraDesk (1:1-Port unter ADR-019) fasst `$4F2`/`$5A0` direkt an. Analog GEMDOS_VERSION/D-083/LP64-Pivot: Contract-/ADR-Zeilen **korrigieren**, nicht nur die Implementierung. Bereits „OFFEN" in `_cookie-jar.md:176-190` + `_boot-init.md`.

### S3 — os_version-Zielwert `0x0404` unverifiziert
`00-PLAN.md` nennt `os_version=0x0404` als feststehend. Nicht EmuTOS-quell-gedeckt (Default `0x0206`, `config.h:1367-1372`); `emutos-config-profile.md` markiert den Wert als „zu verifizieren". GEMDOS meldet 0x3000 (`version.rs:14`, `e162c62`, Selftest prüft). os_version wird gar nicht gemeldet (Teil B1). Zahlenwert = Lead-/Researcher-Entscheidung (Cross-Check reales TOS4). **Anmerkung Lead:** 0x3000 (GEMDOS) + 0x0340 (AES) sind config.h-belegt für TOS v4.0x; 0x0404 (TOS) ist der reale-Falcon-Wert als bewusste Deviation (wie GEMDOS_VERSION) — beim BIOS-Fix in `emutos-config-profile.md` konsistent dokumentieren.

## Acceptable HW-absence Deviations (KEINE Bugs)
- **Bconstat/Bconin/Bconout/Bcostat für Handles 0/1/3/4 (PRT/AUX/MIDI/IKBD-TX):** char_dummy-Semantik (Status 0/Input 0/Output verworfen), nur CON(2) real (`chardev.rs:31-42` ↔ `bios/chardev.c:30-45`). Korrekt.
- **Mediach non-removable:** valider Drive→0, kein Latch/forcechange (SD fest verbaut); EUNDEV-für-invalid korrekt (`mod.rs:159-165`). Korrekt.
- **Drvmap ohne A:/B:-Bits:** keine Floppy → Bits 0,1 clear, HD ab C: (`drive.rs:14-15,71`). Korrekt.
- **Rwabs:** EMMC-SD-Backend, kein Byte-Swap (ARM LE), kein 0x7FFF-Chunking, adr==NULL-Floppy-Feature absent. HW-Adaption.
- **Getmpb:** physischer ST-RAM-Pool statt m68k-membot; read-only, mp_rover bewahrt (matcht EmuTOS); MD-Layout LP64 (24 B). Korrekt.
- **Kbshift:** get/set/-1/return-old + Byte-Truncation exakt (`mod.rs:83-94` ↔ `ikbd.c:145-153`). Korrekt.
- **Tickcal:** konstant 20 (`mod.rs:44,58` ↔ `bios.c:1096`). Korrekt.
- **Dispatcher Out-of-Range:** gibt Funktionsnummer zurück (`mod.rs:69` ↔ `vectors.S:1323-1327`). Korrekt.
- **Cookie-Jar-Inhalte:** Atari-HW-Cookies (_VDO/_SND/_FDC/_FPU) weggelassen (HW-Adaption) — aber der Zugriffs**pfad** ($5A0/Setexc) ist ein echter Gap (B1a).

## Getrieben vs. offen
- **Getrieben (statisch Code↔EmuTOS↔Spec, adversariale Eingaben):** alle 12 Fn Feld-/Rückgabe-/Fehlercode-/ABI-Ebene; Rwabs-Grenzfälle; Getbpb-BPB Feld-für-Feld inkl. Nachweis, dass die BPB `bdos/fs/fat.rs` speist; Setexc/NVEC/Cookie-Slot/Sentinels/etv-Dispatch-Absenz; Boot-Init-Gap (Grep 0 Instanzen + Layout + TeraDesk-Fault); Trap-Routing (`svc #13`→`exceptions.rs:347`→`bios::dispatch`).
- **Offen (PLAUSIBLE, kein Laufzeit-Beleg):** M1–M4 quell-bewiesen, nicht per QEMU reproduziert (bräuchten malformierte/oversize-FAT-Images bzw. injizierten Disk-Fehler); im Normalbetrieb nicht getriggert. B1-Fault bereits in `sysvar-osheader-lowmem-gap.md` reproduziert (CONFIRMED).
