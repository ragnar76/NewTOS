# GH1 — Popup im System-Info-Accessory: Fenster dahinter zeichnet sich ueber den Dialog

Status: **OFFEN — Ursache wird geklaert** · Prioritaet: **P2** · Typ: Fehler — GitHub-Issue #1 (ragnar76, 2026-10-19), echte Pi 4, NewTOS 0.6 (`f7cb0fd9`)

## Meldung

1. Accessory „NewTOS System Information“ (SYSACC) ueber das Desk-Menue oeffnen.
2. Im Dialog ein Popup-Menue oeffnen (Auswahlboxen „Next start“).
3. Das Verzeichnisfenster dahinter (`C:\ACC\`) zeichnet sich ueber den Dialog.
   Erwartet: Fenster bleibt im Hintergrund. Immer reproduzierbar, System laeuft weiter.

Foto: `measurements/measurement-2026-10-19-gh1/issue1-photo.png`
(https://github.com/tuxie01/NewTOS/issues/1).

## Auftrag (Ursache zuerst, kein Fix)

In QEMU raspi4b mit Injektor nachstellen; Ursache mit Beleg (Datei:Zeile)
gegen EmuTOS: wer schickt welche Nachricht (WM_REDRAW?) beim Oeffnen/Schliessen
des Popups, warum zeichnet der Desktop ueber den Dialog (Rechteckliste,
`wind_update`-Sperre, `form_dial`), was tut SYSACC (`sdk/examples/sysacc.c`),
und ob A112 (Abgabe nach jedem AES-Aufruf, `1762e837` alt) die Ursache ist —
Gegenprobe mit einem Kernel vor A112.

[Gemeinsame Regeln](README.md) gelten.
