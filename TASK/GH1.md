# GH1 — Popup im System-Info-Accessory: Fenster dahinter zeichnet sich ueber den Dialog

Status: **OFFEN — Ursache belegt, Fix wartet auf Freigabe** · Prioritaet: **P2** · Typ: Fehler — GitHub-Issue #1 (ragnar76, 2026-10-19), echte Pi 4, NewTOS 0.6 (`f7cb0fd9`)

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


## Ursache (2026-10-19, Auditor, QEMU nachgestellt)

Belege `measurements/measurement-2026-10-19-gh1/RUNINFO.TXT`. Nicht SYSACC,
nicht A112 (vor A112 dasselbe Bild).

1. AES `post_button` erfuellt die wartenden Knopf-Events **aller** Prozesse
   statt nur des Maus-Besitzers (`tos-api/src/aes/input.rs:499-552`; EmuTOS
   `aes/geminput.c:463-483`). Jeder Klick im Accessory-Dialog erreicht auch
   NEWDESK. Spec `docs/api/aes/event.md:524-526` nennt das als Naeherung.
2. AES `wind_update` blockiert nie, und die Menue-Steuerung haelt keinen Lock
   bis zum Loslassen (`window.rs:4167-4195`, `event.rs:158-238`; EmuTOS
   `gemflag.c:68-129`, `gemwmlib.c:1778-1797`, `gemctrl.c:463,484`) — deshalb
   laeuft `fm_own` des Accessorys bei gedrueckter Taste, der Maus-Besitz geht
   nicht ans Accessory. Spec `window-graphics.md:1132-1135`,
   `accessories.md:873` („nothing observes it“) widerlegt.
3. NEWDESK: `wind_update(BEG/END_UPDATE)` um den Event-Handler fehlt
   (`programs/desktop/src/main.rs:84-107`; EmuTOS `desk/deskmain.c:2094,2121`).
4. AES `wind_set(WF_INFO/WF_NAME)` zeichnet den ganzen Rahmen statt nur des
   Elements (`window.rs:3976-3983,4122`; EmuTOS `gemwmlib.c:1668-1674,1754-1757`).

Fix-Vorschlag: F1 echter `wind_update`-Mutex (Wartende schlafen, Menue haelt
den Lock bis Loslassen), F2 `post_button` nur an den Maus-Besitzer, F3 Klammer
in NEWDESK, F4 Element-Neuzeichnen — F1–F3 nur zusammen (F2 allein verschiebt
das Symptom, im Lauf belegt).
