# PRESENT01 — Quittung (FERTIG in QEMU, ein offener Punkt): PRESENT.NAP, Folien im Vollbild

Datum 2026-10-08 · Basis `desk-rebuild` `4d4c218` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-08-present](../measurements/measurement-2026-10-08-present/RUNINFO.TXT)
· Ticket: [PRESENT01](PRESENT01.md) · Doku: [docs/sdk/14-present.md](../docs/sdk/14-present.md)

## Ergebnis

`PRESENT.NAP` liest `SLIDES.TXT` (Argument, sonst aktueller Ordner, sonst
`C:\APPS\PRESENT\SLIDES.TXT`), übernimmt den ganzen Bildschirm
(`BEG_UPDATE` + `BEG_MCTRL`, `form_dial(FMD_START)`, `graf_mouse(M_OFF)`) und
zeigt je Folie: Titel mit oranger Linie, Punkte mit orangem Punkt,
Unterpunkte mit Strich, Absätze, optional ein PNG rechts (oder allein über die
ganze Fläche), unten links eine Fußzeile (`@footer`), unten rechts „n / N".
`@center` = zentrierte Titelfolie. Beim Ende `M_ON`, `FMD_FINISH`,
`END_MCTRL`, `END_UPDATE` — der Desktop ist wieder da.

Große Schrift: die Zeichen 32–255 des Systemfonts werden einmal mit `v_gtext`
gezeichnet und per `vro_cpyfm` in ein Speicher-MFDB (RGB565) zurückgelesen,
dann mit Scale2x/Scale3x vergrößert (×2, ×3, ×4, ×6, ×8) und je Größe
zwischengespeichert — runde Diagonalen, keine Lücken. Eine Folie wird in einem
RGB565-Bild aufgebaut und mit einem einzigen `vro_cpyfm` gezeigt. Größen nach
Bildschirmhöhe (1080: Titel ×4, Punkte ×3, Unterpunkte ×2); passt eine Folie
nicht, wird ihre Punktgröße eine Stufe kleiner. PNG über libpng (statisch),
Alpha mit dem Hintergrund verrechnet, auf ganze Vielfache vergrößert oder mit
Boxfilter verkleinert, je Folie zwischengespeichert; fehlend/kaputt → kleiner
Hinweis `[picture not found: NAME]`. UTF-8 wird ins Atari-Zeichensatz
umgesetzt (Tabelle aus EmuTOS `tools/bug.c`).

Tasten: weiter = Rechts, Runter, Leertaste, Return, Enter, N, Linksklick;
zurück = Links, Hoch, Backspace, P, Rechtsklick; ClrHome = erste,
Shift+ClrHome = letzte; Esc/Q = Ende.

## Dateien

* `sdk/apps/present/present.c` (neu) — das Programm, Format im Kopfkommentar
* `sdk/apps/present/build-present.sh` (neu) — `build/PRESENT.NAP` + `build/TEST.PNG`
* `sdk/apps/present/sample/SLIDES.TXT`, `sample/gen-sample.py` (neu) — Beispielfolien, Testbild
* `sdk/.gitignore` — Quellen von `sdk/apps/present` werden verfolgt (wie chaintest)
* `tools/make-sd-image.sh` — installiert `C:\Apps\PRESENT\` (nur QEMU-Abbild)
* `docs/sdk/14-present.md` (neu), `docs/sdk/README.md` — Doku + Index

Lieferung: `measurements/measurement-2026-10-08-present/bin/` — `PRESENT.NAP`
(sha256 `94ebf10b2a5f8aaeba2aa2c89fdf2bde5cf161378f55013e874e26eef536ec29`),
Beispiel-`SLIDES.TXT`, `TEST.PNG`, `LIESMICH.TXT`.

## Prüfung (QEMU raspi4b, `serial=on`)

* 640x480, 1024x768, 1920x1080 und 1920x1080 mit `memprot=strict`, Start per
  `#Z`, Injektor: Rechts → Leertaste → Linksklick → Links → Shift+ClrHome →
  ClrHome → Rechts → Rechtsklick → Esc. Jeder Screendump zeigt die erwartete
  Folie; `PRESENT.LOG` zählt Folie 1 dreimal, 2–4 je zweimal (= die Folge);
  danach EmuDESK mit Maus. 0 `[EXC]`/`[PANIC]`. Strict pixelgleich zu normal.
* Zeiten 1920x1080 (QEMU-Gastzeit, 5-ms-Takt): Aufbau ≤ 5 ms, Blit 15–20 ms,
  PNG laden+skalieren 40–45 ms (einmal).
* APITEST 106 / 0 / 5 wie zuvor. Build ohne Warnung, 0 undefinierte Symbole;
  zwei Builds bytegleich. Kein Rust-Code berührt.

## Offen

* **Bild auf / Bild ab / Ende kommen von einer USB-Tastatur nicht an**:
  `kernel/src/drivers/usb/hid.rs` `translate_key` liefert für USB 0x4B/0x4D/0x4E
  nichts. Präsentations-Klicker, die Bild auf/ab senden, wirken daher nicht;
  Klicker mit Pfeiltasten gehen. Behebung bräuchte eine Kernel-Änderung
  (Scancode-Wahl belegen) — nicht Teil dieses Tickets.
* Nicht auf der echten Pi gelaufen; Start per Doppelklick aus dem Desktop
  nicht eigens getestet (nur `#Z`); kein Host-Test für Parser/Umbruch.
