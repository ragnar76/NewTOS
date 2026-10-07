# MEMPROT-FAST — Quittung: Speicherschutz-Sync rechnet nur, wo sich Rechte ändern

Datum 2026-10-07 · Basis `desk-rebuild`, HEAD `438e102`, Baum vorher sauber
· nicht committet · Belege
[measurements/measurement-2026-10-07-memprot-fast/](../measurements/measurement-2026-10-07-memprot-fast/RUNINFO.TXT)
· Ticket [MEMPROT-FAST](MEMPROT-FAST.md)

Umfang: Fix-Optionen 1 und 3 aus
`docs/analysis/zview-sluggish-2026-10-07.md` §1 (im Auftrag „1+2“ genannt):
nur dort und für die Prozesse neu rechnen, wo sich Rechte ändern; Änderungen
einer zusammengesetzten Operation einmal eintragen. 2-MiB-Blockdeskriptoren
(Analyse-Option 2) **nicht**.

## Bau

`tos-api/src/bdos/process/memprot.rs` (Moduldoku „Tables“):

* Invariante bleibt „berechnet, nicht fortgeschrieben“: ein Ereignis nennt nur
  Bereich + Slots (+ „Nicht-Besitzer-Muster“, + „Slots mit privater Kopie“),
  `sync` rechnet diese aus Poollisten, Besitzern, Sätzen, E10 neu
  (FreeMiNT-Abkürzung `mprot040.c:1102-1130`).
* gleicher Modus / `PROT_P`↔`PROT_S` → nichts; Nicht-Besitzer-Muster nur bei
  `PROT_G`/`PROT_PR`; Besitzerwechsel nur über die Besitzer-Cookies, die
  kommen/gehen (`on_owner_change_by`, aus `region.rs` und `set_owner`);
  Freigabe/`Mshrink` nur der zurückgegebene Teil (`on_released`);
  Strukturänderungen (`on_root_built`, `set_space` für `Pexec(4)`,
  `on_slot_released`, E10) als Differenz je Block; Vollneubau nur für eine
  frische Wurzel. Unangekündigte Strukturänderung → Vollneubau aller Slots
  (gezählt, in allen Kernel-Läufen 0).
* `memprot::batch()` in `Pexec` (laden/starten/Basepage), `Pterm`-Kehraus und
  ausgerichteten Allokationen; `install_ttbr0` und Trap-Ende `sync_now`.
* TLB-Pflege unverändert über `set_leaf`/`make_private`/`drop_private`.
* Prüfmodus `memprot=verify` (Kernel) / `set_verify` (Hosttests, Panik):
  unabhängige Vollberechnung nach jedem Sync und nach jedem Rechte-Ereignis.

Übrige Dateien: `mmu.rs` (`install_ttbr0` → `sync_now`), `pexec.rs`,
`pterm.rs`, `mem/traps.rs`, `mem/region.rs`, `kernel/src/bootargs.rs`,
`kernel/src/lib.rs`, `kernel/src/arch/aarch64/exceptions.rs`.

## Prüfungen

| Prüfung | Baseline | Jetzt |
|---|---|---|
| Hosttests | 1235/0/1 | **1248/0/1** (+13 `memprot_fast.rs`) |
| neue Tests, Arbeit wie Stufe 3b (Kratzkopie) | — | 9/13 rot, Korrektheit grün |
| Prüfmodus in den vier memprot-Testdateien | — | an (Panik), grün |
| Release rpi4 / qemu-virt (tos-api/kernel) | 64/94 | 64/94 |
| Clippy rpi4 / virt | 76/125 | 76/125 |
| Drift | 31 | 31 |
| SDK-Hosttests | 18/0/0 | 18/0/0 |

QEMU mit `memprot=verify`: ARCH03-4-Reihe in on/off/strict/on-asid wie
ARCH03-4 (nur PORT10-Kartendefekt, Wiederholung ALL PASS ×4); LDGTEST,
LDGDEMO-Popup, LDGDEMO-Abnahme, zView-Start in vier Modi, bytegleich zu HEAD.
152 Läufe, 0 Abweichungen, 0 Rückfälle.

## Messung (QEMU, relativ)

| zView-Start (hd) | vorher | nachher |
|---|---|---|
| memprot | 1331 / 1340 ms | 549 / 557 ms |
| Mausstillstand | 1524 / 1568 ms | 787 / 793 ms |
| Pexec 3 / 4 memprot je Aufruf | 49–50 / 47–52 ms | 17–18 / 17–18 ms |
| Pexec 5 | 5–6 ms | 0 ms |
| Mshrink (11×) | 242 ms | 160 ms |

Rest = echte Änderungen (TPA ~916 MB: zView nach `PE_LOAD`, das Kind nach
`Pexec 4`, beide verlieren sie beim `Mshrink`); weg erst mit
2-MiB-Blockdeskriptoren.

## Offen

* Pi-Lauf: `measurements/measurement-2026-10-07-memprot-fast/bin/`
  (`kernel8.img` `13a5e906…`, `LIESMICH.TXT`).
* Beobachtung ohne memprot-Bezug: `evnt_multi`-„Arbeit“ im ZVPROF-Profil
  höher (Busy-Wait-Schleife, zeitlich durch den 20-ms-Timer begrenzt;
  Diagnosebau ohne memprot-Funktion darin), `vro_cpyfm` schwankt zwischen
  Bauten (Code-Layout unter TCG) — RUNINFO.
