# DESK11 — Desktop liest die Laufwerksliste mit `Drvmap`, EmuTOS mit `Dsetdrv(Dgetdrv())`

Status: **OFFEN** · Prioritaet: **P4** · Typ: Konformitaet desk/ — Nebenbefund DESK10

Abhaengigkeiten: [DESK10](DESK10.md) (erledigt)

## Sachverhalt

EmuTOS' Desktop holt die Laufwerksmaske ueber GEMDOS: `dos_sdrv(dos_gdrv())`
= `Dsetdrv(Dgetdrv())`, dessen Rueckgabe die Bitmaske ist
(`references/emutos/desk/deskapp.c:714`). NewTOS ruft BIOS `Drvmap` (0x0A)
— dieselbe Bitmaske aus derselben Tabelle (`tos-api/src/bios/mod.rs:223-224`,
`bios/drive.rs:145`), aber ein anderer Aufruf. DESK10 hat den Aufruf auf den
Trap umgestellt, ohne zu aendern, **welcher** Aufruf es ist (Auftrag lautete
„Verhalten unveraendert").

## Arbeit

Auf `dos_sdrv(dos_gdrv())` umstellen (`dosbind.rs` hat die Bindung noch
nicht — zwei Zeilen), Aufrufstellen `app.rs`, `main.rs`, `mod.rs`;
Bootprotokoll und Laufwerksliste unveraendert belegen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
