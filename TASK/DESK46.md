# DESK46 — Konsolenprogramme nach U3: Bildschirm blitzt weiss, und aus EmuCON heraus friert es ein

Status: **OFFEN** · Prioritaet: **P1** · Typ: Belegter Defekt — Pi-Runde 2026-09-26

Abhaengigkeiten: [DESK43](DESK43.md) U3/U4 (`e0069e5`, `5a485be`) — der Konsolenarm wurde dort umgebaut

## Was auf der Pi lief (damit der Rahmen stimmt)

Kern `14065b06`. **Funktioniert:** QED per Doppelklick starten und beenden;
TeraDesk laeuft; QED **aus TeraDesk heraus** starten und sauber beenden; die
offenen EmuDesk-Fenster werden nach der Rueckkehr **rekonstruiert**. Damit sind
U3s Hardware-Punkte 1 und 2 bestanden.

## Befund 1 — `.NCP` vom Desktop: kurz weiss, dann weg

Anwender: *„beim Starten der NCP-Apps, da wird das Bild kurz weiss und dann
weg."* Auf die Rueckfrage, ob im Weiss **Text** stand: *„zu schnell, kann ich
nicht sagen."*

**Beides ist moeglich und noch nicht getrennt:**
* Es ist die Folge der Entscheidung vom 2026-09-26 — `console_pause_for_key`
  ist entfallen (EmuTOS-konform), also erscheint die Ausgabe und der Desktop
  kommt sofort zurueck. Dann ist es **kein Defekt**.
* Oder der Textschirm wird betreten und sofort wieder verlassen, **ohne** dass
  das Programm zum Zeichnen kommt. Dann ist es einer.

**Zu messen, nicht zu raten:** ein Screendump im Moment des Konsolenschirms,
bevor der Desktop zurueckkehrt.

## Befund 2 — aus EmuCON heraus friert es ein

Anwender: *„dann bin ich in EmuCon gegangen, was auch funktioniert; dort konnte
ich `sysinfo.ncp` aufrufen, es hat die Ausgabe gebracht und dann war er
eingefroren."*

**Das ist ein Defekt.** Die Ausgabe kommt vollstaendig — das Programm laeuft
also —, und **danach** steht es.

Nicht erhoben, aber fuer die Eingrenzung noetig: ob die **Maus** dabei noch
lebt. Bei PORT05 war genau das der Trenner — tote Maus heisst unterhalb der
AES, lebende Maus heisst eine Anwendung schlaeft in einem AES-Aufruf
([[mouse-moves-but-nothing-responds-means-app-sleeping]]). In EmuCON ist der
Zeiger allerdings ohnehin abgeschaltet, die Frage ist also anders zu stellen:
reagiert die **Tastatur** noch?

## Ticketnachbarschaft, als Verweis und nicht als Diagnose

* U3 hat den Konsolenarm umgebaut: `launch_native`s Console-Arm,
  `sh_run_pending_console` und `sh_alpha_enter`/`sh_alpha_leave` sind
  **geloescht**; der Weg laeuft jetzt ueber `sh_main`s `CONSOLE_APP`-Arm
  (`gemshlib.c:614-622`).
* EmuCON startet ein Programm als **EL0-Anwendung, die eine EL0-Anwendung
  startet** — dieselbe Familie wie [PORT05](PORT05.md), nur auf dem
  Konsolenarm statt dem GEM-Arm. PORT05 ist behoben und auf Hardware
  abgenommen; ob hier etwas Verwandtes offen ist, ist **ungemessen**.
* `console_pause_for_key` ist entfallen (Anwenderentscheidung 2026-09-26). Das
  betrifft Befund 1, nicht Befund 2 — bei Befund 2 friert es ein, statt
  zurueckzukehren.

## Auftrag

1. **Befund 1 trennen**, headless: Screendump des Konsolenschirms waehrend
   eines `.NCP`-Starts vom Desktop. Steht dort die Ausgabe, ist es das
   gewaehlte Verhalten und das Ticket schrumpft auf Befund 2.
2. **Befund 2 nachstellen**, headless: EmuCON starten, darin ein `.NCP`
   ausfuehren. Tritt es in QEMU nicht auf, sagen was sich unterscheidet.
3. Erst wenn feststeht, **wo** es stehenbleibt, nach der Ursache suchen.

## Warum P1

„Nichts darf haengen" ist die Vorgabe fuer den Oktober-Termin, und ein
eingefrorenes System nach einem Konsolenprogramm ist genau das.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
