# ACCTITLE — Quittung (FERTIG): Accessory-Titel im Desk-Menü einer Anwendung sind für die Anwendung lesbar

Datum 2026-10-07 · Basis `desk-rebuild` `3895575` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-07-acctitle](../measurements/measurement-2026-10-07-acctitle/RUNINFO.TXT)

## Befund (echter Pi 4, Nutzer)

zView (ZVIEW.NAP `9c6497ad`) mit geladenem `C:\ACC\SYSACC.NAC`: JPEG laden,
Vollbild, Esc → „Memory violation: hardware memory, read (permission fault,
level 2)“, `esr=9200000e`, `addr=ffff000000193af0`, `pc=text+0x489a4`,
`lr=text+0x1cd64`.

## Kette (geprüft)

* `ffff000000193af0` = `tos_api::aes::menu::ACCS` (Größe 0x130), Feld der
  Titelkopie von Slot 0; `pc` = `strstr`+8, `lr` in WinDom `menu_exec_cmd`.
* WinDom `menu_exec_cmd` durchsucht bei jeder nicht anders behandelten Taste
  alle Einträge ab dem Desk-Menü per `strstr` — auch die Accessory-Einträge.
* EmuTOS: `mn_register` speichert den Zeiger (`gemmnlib.c:806`), `menu_fixup`
  hängt ihn in den Baum (`:118`); der String ist normaler Speicher, den die
  Anwendung frei liest.
* NewTOS `HEAD`: `menu_fixup` schrieb die **Kernel**-Adresse der Titelkopie in
  jeden Baum, auch in den der Anwendung → Lesezugriff aus EL0 = Fehler. Dazu
  zeichnete die AES diese Einträge in App-Menüs leer (TeraDesk/QED, belegt).

## Was geändert ist

* `tos-api/src/aes/menu.rs`: die Titelkopien liegen in einer eigenen Seite
  (`TITLES`, `TitlePage`, 4096-ausgerichtet, sonst nichts darin), die beim
  ersten Bedarf **schreibgeschützt für alle Prozesse bei VA = PA**
  veröffentlicht wird (`usermem::expose`, `PROT_PR`, ADR-023, Muster der
  PPARENT-Stellvertreterseite). `menu_fixup` schreibt in den Baum einer
  Anwendung die Benutzeradresse, in den Baum des Kernel-Desktops weiter die
  Kernel-Adresse (`title_spec`). Neuer Hosttest `acc_title_tests`.
* Spec: `docs/api/aes/accessories.md` §9.3 (a) und §9.4 (der dort als „nicht
  gemacht, nie ein Fehler“ geführte Punkt ist erledigt und korrigiert),
  `docs/api/aes/menu.md` Hinweis 6.

## Durchsicht weiterer Stellen derselben Art (AES/VDI)

Einzige Stelle, die einen Kernel-Zeiger in eine für die Anwendung sichtbare
Struktur schreibt, war `menu_fixup`. `wind_get(WF_SCREEN)` und `shel_envrn`
veröffentlichen bereits, der AES-Globalbereich trägt keine Kernel-Zeiger,
`PARMBLK` liegt in der Page-0-Scratch, Alert-/Fsel-Bäume werden nie
herausgegeben, VDI `vex_*` liefert Benutzer-Token. Einzelheiten mit Datei:Zeile
im RUNINFO.

## Messwerte

| Messung | Soll | Ist |
|---|---|---|
| QEMU-Sonde, `HEAD`, on + strict | Fehler | **Fehler** wie auf dem Pi (`esr 9200000e`, `addr ffff000000193af0`) |
| QEMU-Sonde, Fix, on + strict | lesbar | **„ NewTOS System“** ×2, rc=0 |
| zView F10/Esc/Esc/F10/Esc, `HEAD` | — | Absturz Register für Register wie beim Nutzer |
| zView dto., Fix, on + strict | kein Absturz | **kein Absturz**, Vollbild hin und zurück |
| EmuDESK Desk-Menü + Accessory öffnen | wie HEAD | gleich (Unterschiede nur Mauszeiger) |
| TeraDesk / QED Desk-Menü | Titel sichtbar | **sichtbar** (HEAD: leere Einträge); Alert öffnet |
| LDGDEMO | wie K1 | 10/10 bytegleich |
| APITEST | 106/0/5 | **106/0/5** |
| Hosttests | 1175/0/1 + neu | **1176/0/1** |
| Release-Bau rpi4 / qemu-virt | 64/94 | **64/94** |
| clippy rpi4 / qemu-virt | 76/125 | **76/125**, keine Treffer in geänderten Zeilen |
| `desk-drift-probe.sh` | 31 | **31** |
| SDK-Hosttests | 16/0/0 | **16/0/0** |

## Offen

* Echter Pi 4 nicht gefahren (keine HW-Aktionen durch Worker).
* Eine Anwendung kann den Titel lesen, aber nicht mehr überschreiben (unter
  EmuTOS ist er Speicher des Accessorys); kein Programm bekannt, das das tut.
