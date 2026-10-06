# ARCH03 Stufe 4 — ASIDs und TLB-Feinpflege — Quittung

**Status: UMGESETZT hinter dem Bootargument `asid=on` (Standard aus) · in QEMU
auf beiden Boards in allen Schutzmodi × ASID aus/an/tiny belegt · Abnahme
durch den Tester offen · Pi offen (nur auf Ansage)** · 2026-10-06 · Zweig
`desk-rebuild`, Basis HEAD `c0ed62b`, Baum vorher sauber, nichts committet ·
Belege [measurements/measurement-2026-10-06-arch03-4/](../measurements/measurement-2026-10-06-arch03-4/)
(`RUNINFO.TXT`, `checks/`, `serial/`, `screendumps/`, `method/`, `bin/`).

## Was gebaut ist

| Bereich | Datei / Funktion |
|---|---|
| Zuteilung | `tos-api/src/bdos/process/asid.rs` (neu): `Allocator` — ASID je **Belegung** eines Slots, aufsteigend, nie zweimal je Generation, ASID 0 = leere Wurzel; Überlauf → neue Generation (Aufrufer muss alles verwerfen); `retire`. `Queue`/`Inval` — geschuldete `TLBI VAE1IS`/`VAAE1IS` mit `Xt`-Format (ASID [63:48], VA[55:12] in [43:0]) |
| Tabellen | `tos-api/src/bdos/process/mmu.rs`: `nG` auf jedem Anwenderblatt (nur `asid=on`); `configure_asids`, `asids_enabled`, `asid_stats`; `install_tagged` (`dsb ish; msr ttbr0_el1, L0│ASID<<48; isb`, kein TLBI; Überlauf: leere Wurzel/ASID 0 → `tlbi vmalle1is` → neue Wurzel); `set_leaf(…, va, owner)` schuldet `VAAE1IS` (geteilte Tabelle) bzw. `VAE1IS`+Besitzer-ASID (private); `flush_tlb` gibt die Schuld aus (> 64 → `vmalle1is`); `replace_bbm` per `VAE1IS` der Region mit Slot-ASID; `drop_private` schuldet abweichende Seiten; `init_slot_tables`/`flush_tlb_slot_released` ziehen die ASID ein (`ASIDE1IS`). ASIDs aus: alter Pfad unverändert |
| Rechte | `tos-api/src/bdos/process/memprot.rs`: Aufrufe von `set_leaf`, `on_slot_released` |
| Kern | `kernel/src/arch/aarch64/mmu.rs` `size_asids` (`ID_AA64MMFR0_EL1.ASIDBits` → `TCR_EL1.AS`, danach `tlbi vmalle1` nach `RVNRFW`); `user_entry.rs` `restore_boot_ttbr0` ohne TLBI bei `asid=on`; `exceptions.rs` `handle_pexec` speichert `parent_l0_pa` ohne ASID-Bits; `lib.rs` `asid_switch`/`boot_switch` (Bootargumente einmal lesen; `memprot_switch` nutzt denselben Leser), Konfiguration direkt nach `reinstall`; `bootargs.rs` `asid=on`, `asid=tiny` |
| Tests | `tos-api/tests/arch03_4_asid_alloc.rs` (8), `arch03_4_asid_on.rs` (2), Unit-Tests in `asid.rs` (2), `bootargs.rs` (1) |
| Doku | Plan: „Stufe 4 wie gebaut“ + E12; `docs/MEMORY.md` QUIRK-001 (Status + Nachtrag), ADR-010 ASID-Punkt; `TASK/ARCH03.md` Zeile 4 |

`asid=tiny` = an, aber nur 3 ASIDs je Generation (Prüfhaken für den Überlauf).

## Prüfungen

| | Basis (proc02) | jetzt |
|---|---|---|
| Hosttests | 1152/0/1 | **1165/0/1** (+13 neu) |
| Release-Bau rpi4 / qemu-virt (Warnungen tos-api/kernel) | 64/94 | 64/94 |
| Clippy rpi4 / virt | 76/125 | 76/125, je Datei identisch |
| Drift | 31 | 31 |
| SDK-Hosttests | 13/0/0 | 13/0/0 |

QEMU-Reihe (`method/series-arch03-4.sh`, die volle PROC02-Reihe je Modus +
ASID-Sonden) in `on off strict on-asid off-asid strict-asid on-tiny`, dazu
`off-tiny`/`strict-tiny` verkürzt: alles PASS und je Modus deckungsgleich mit
ASID aus bzw. mit proc02 — einzige Abweichung PORT10 1/44 in fünf Modi durch
eine **0-Byte-Datei auf der gesäten Karte** (macOS-FAT, nicht der Kern);
Wiederholung auf geprüfter Karte: ALL PASS (44) in allen fünf. Erzwungener
Überlauf: 80 bzw. 31 Generationen je Lauf, alles PASS.

Sonden (`method/tests/asid*.c`): ASIDSTR (Seite 0 über 32 Pexec-Kinder, 4
Ketten à 6 Generationen, Inaktiv-ASID-Verengung, 200 Wartevorgänge gegen das
Accessory ASIDACC), ASIDPERF (Zeitmessung). **Gegenproben** mit absichtlich
kaputten Kernen: ohne TLBI und ohne ASID fällt ASIDSTR (101/200 fremde Seite 0);
ohne Feinpflege fällt MPROT `f`. **Aber:** der Inaktiv-ASID-Fall (ASIDSTR Teil
C) besteht auf QEMU auch ohne Feinpflege — QEMU macht Einträge inaktiver ASIDs
nicht beobachtbar.

## Wechselkosten (QEMU, nur Anhalt)

Mit `asid=on` ist der Wechsel auf QEMU **teurer** (Installation 1,7 → 5,8 µs
bei Schutz an; `restore_boot_ttbr0` ohne TLBI kostet gleich viel wie mit) —
jeder ASID-Wechsel in `TTBR0` kostet QEMU offenbar einen TLB-Flush [Schluss].
Teil A (Pexec-Schleife) mit Schutz an wird ~20 % schneller (Feinpflege statt
`vmalle1is`). Der Nutzen auf Hardware ist ungemessen. Tabelle:
`checks/swcost-a4.txt`.

## Empfehlung Standardwert

**Aus lassen**, bis ein Pi-Lauf mit `asid=on` **und** `asid=tiny`
(ASIDSTR/ASIDPERF mit Accessory, MPROT3B, S2ALL, Desktop/TeraDesk/QED, Zählerkern
`kernel8-asidk` für die Wechselkosten) sauber ist; auf QEMU bleibt aus
ohnehin günstiger. Danach Standard an für `board-rpi4`, und die QUIRK-001-Klammer
(Druckzeilen zwischen den Primitiven, Invariante I9) streichen.

## Nur auf der echten Pi 4 prüfbar

1. Behalten von TLB-Einträgen über ASIDs hinweg — also ob die Feinpflege für
   **inaktive** ASIDs stimmt (ASIDSTR Teil C, Seite 0 unter anderer ASID);
2. `TCR_EL1.AS` zur Laufzeit umschalten;
3. der Überlauf-Flush auf Hardware;
4. Kosten/Nutzen des Wechsels ohne TLBI;
5. die ganze Reihe mit `asid=on`/`asid=tiny`.

## Offen / Beobachtungen außerhalb des Auftrags (unberührt)

- Kartensaat: `seed-card.sh` hinterlässt auf macOS mitunter `C:\P10\ONLYNCP.NCP`
  mit 0 Byte nach dem Aushängen (beim Einhängen noch richtig) — betrifft jede
  PORT10-Messung; `method/rerun-port10.sh` prüft und schreibt nach.
- Mit Schutz an kostet ein `Pexec` eines kleinen Programms auf QEMU ~200 ms
  (ohne Schutz ~5 ms) — schon im proc02-Kern so (Basislauf in `serial/`).
- E9MEM-`_PgmSize` schwankt zwischen Läufen um 16 Byte (auch bei ASID aus,
  proc02 vs. jetzt) — Ursache nicht gesucht.
- Die QUIRK-001-Druckzeilen und der Umweg über die leere Wurzel bleiben auch
  mit `asid=on` (auf der Pi ein `msr` ohne Flush; Entfernen eigener Schritt).
- `docs/STATUS.md` nicht angefasst (Lead).
