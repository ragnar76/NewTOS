# ACC02 — STATUS: DONE (2026-09-25) — Pi-Sichtabnahme offen

**DONE bedeutet hier: gelandet und in QEMU raspi4b belegt; die Sichtabnahme
auf der echten Pi 4 steht aus (Tester/Lead; nichts geflasht).** Bearbeitet
2026-09-25 als **eine** Arbeitseinheit mit ACC02, ACC03, ACC04 und ACC05 — ein
Messverzeichnis, `measurement-2026-09-25-acc02-acc04/`. Branch
`desk-rebuild`, Basis HEAD `9a346a4`, nichts committet, kein
`git checkout/restore/clean/stash/reset`.

Die Begruendung im Einzelnen steht im Abschnitt **Abschluss** von
[ACC02.md](ACC02.md); die Umsetzungsnotizen mit jeder benannten Abweichung in
`docs/api/aes/accessories.md` §9.

## Nachweis

* `measurement-2026-09-25-acc02-acc04/RUNINFO.TXT` — die Laeufe, die
  Koordinaten und die zwei Defekte, die dieser Stapel an sich selbst fand.
* `screendumps/` — zwoelf Bilder aus einem Produktstand.
* `serial/` — vier Serienprotokolle, null `[EXC]`, null `[PANIC]`.
* `build/fsck.txt` — `fsck_msdos -n` auf allen vier Kartenabbildern, sauber.

## Offen

* Sichtabnahme auf der echten Pi 4 (Menue, Alert, Bedienbarkeit daneben).
* Accessory-Titel im Menuebaum einer **Anwendung** — eigenes Ticket, siehe
  `docs/api/aes/accessories.md` §9.4.
