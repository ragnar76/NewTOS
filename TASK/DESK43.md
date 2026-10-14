# DESK43 — Der Desktop muss im User-Space laufen, wie jede andere Anwendung

Status: **OFFEN** · Prioritaet: **P1** · Typ: **Abweichung von EmuTOS** (nicht blosse Architekturfrage) — Anwenderentscheidung 2026-08-31, geschaerft 2026-09-01, belegt 2026-09-25

Abhaengigkeiten: **[PORT05](PORT05.md) muss vorher stehen** (Begruendung unten).
Plan liegt: `docs/analysis/desktop-userspace-plan.md` (68 KB, 2026-09-01)

## Einordnung des Anwenders 2026-09-25

*„Das ist direkt das naechste nach dem Scrollwheel-Task. Das ist wichtiger als
alles andere."* — damit **P1**, und die Reihenfolge ist: [DESK42](DESK42.md)
(laeuft) → [GUI04](GUI04.md) (Mausrad) → **dieses Ticket**.

**Mit einer Einfuegung, die technisch zwingend ist:** [PORT05](PORT05.md)
gehoert **davor**. Zieht der Desktop nach EL0, startet er Anwendungen ueber
genau den Weg, der laut PORT05 einfriert — heute umgeht er ihn, weil er
kernelseitig startet, und das ist der einzige Grund, warum QED aus NewDesk
laeuft. Der Umzug vor dem Fix wuerde den Desktop handlungsunfaehig machen.
PORT05 ist also keine Konkurrenz zu diesem Ticket, sondern seine
Voraussetzung.

## Warum dieses Ticket ueberhaupt existiert

Die Entscheidung ist **ein Jahr alt in Projektzeit und nie eingeplant worden**,
weil es kein Ticket gab — nur einen Gedaechtniseintrag und einen fertigen Plan.
Der Anwender hat am 2026-09-25 von selbst wieder danach gefragt: *„aber wieso
ist unser Desktop nicht auf einer EL0-Anwendung? Das sollte eigentlich so
sein."*

## Ist-Zustand

`desk::run()` haengt direkt im Boot-Pfad (`kernel/src/lib.rs:329`);
`desk/mod.rs:159` nennt sich selbst *„the desktop's kernel-boot context"*.
Der Desktop ist Kernel-Code mit Oberflaeche, kein Prozess.

Gemessener Ausgangspunkt (2026-09-01): `kernel/src/desk/` = **9 234 Zeilen in
15 Dateien**; nur **sechs** direkte `crate::`-Abhaengigkeiten zum Kernel, aber
rund **40 direkte `tos_api::`-Aufrufe am Trap vorbei** (`aes::object` 9,
`bdos::fs` 9, `vdi::font` 6, `host::host` 6, `aes::window` 6, `bios::drive` 3,
`vdi::mouse` 1). Das sind die Stellen, die der Umzug geradezieht.

## Belegt 2026-09-25: EmuTOS' Desktop laeuft im User-Modus

Der Anwender fragte: *„aber im EmuTOS laeuft er doch auch im User-Space?"* —
und das stimmt, nachgelesen statt angenommen:

* `aes/gemasm.S:77-79` — die AES springt in das TEXT-Segment eines
  Accessories *„in user mode"*.
* `aes/gemstart.S:126` — *„Super() to go back to user mode"*.
* `aes/gemshlib.c:604,634` — beim Start einer Anwendung uebernimmt diese
  **denselben** Deskriptor `rlr`, den der Desktop hatte.

**Damit ist `desk/` auf EL1 eine Abweichung von EmuTOS, keine
Entwurfsfreiheit.** Nach [Don't rationalize EmuTOS deviations] ist die
Vorgabe dann *angleichen*, nicht begruenden — dieses Ticket ist also kein
„waere schoener", sondern eine offene Konformitaetsluecke.

**Ein Unterschied bleibt und bestimmt den Aufwand:** EmuTOS hat **keinen**
Speicherschutz. „User mode" heisst dort nur S-Bit aus, alles liegt in einem
flachen Adressraum. Unser EL0 bringt Modus **und** eigenen Adressraum mit.
Die Richtung ist EmuTOS-konform; der Weg dorthin ist bei uns teurer, weil die
rund 40 Aufrufe am Trap vorbei echte Adressraumgrenzen ueberqueren muessen,
die es dort nicht gibt.

## Was sich damit von selbst aufloest

In EmuTOS **ist** der Desktop der AES-Prozess `rlr`, und eine gestartete
Anwendung uebernimmt **denselben** Deskriptor (`sh_ldapp` ruft
`p_nameit(rlr, fname)`, `aes/gemshlib.c:622-634` — kein `pstart`/`getpd`).
Daher misst EmuTOS `ap_id = 0` fuer die gestartete Anwendung.

Zwei gemessene Abweichungen sind **reine Symptome** davon und duerfen nicht
vorher einzeln „behoben" werden — das waere eine hingeschriebene Zahl statt
nachgebautem Verhalten:
* `ap_id` 1 statt 0 (D-R1-5)
* `ap_private` aus dem Fallback statt der DESKTOP-Ressource (D-R1-6)

Ebenso faellt unser `save_windows`/`restore_windows`-Handoff: EmuTOS'
`sh_ldapp` schliesst **keine** Fenster. Der Handoff existiert nur, weil unser
Desktop strukturell etwas anderes ist.

**Nebeneffekt, der uns nuetzt:** der Umzug beendet den In-Kernel-Zeichenpfad,
wegen dem ein gut aussehender Desktop den EL0-Anwendungspfad bisher **nicht**
validiert.

## Beruehrungspunkt mit [PORT05](PORT05.md)

PORT05 (QED aus TeraDesk gestartet friert ein) ist der erste Fehler, der
**wegen** dieser Struktur auffaellt: der Weg „EL0-Anwendung startet
EL0-Anwendung" ist bei uns nie gegangen worden, weil der Desktop ihn nicht
geht. **PORT05 wartet aber nicht auf dieses Ticket** — dieser Weg muss
funktionieren, ganz gleich wo der Desktop lebt, und vor dem Oktober-Termin ist
der Umzug zu gross. Erst PORT05, dann hier weiter.

## Die wichtigste Randbedingung

**Es ist der dritte Anlauf.** Zwei Rebuilds sind vorher abgedriftet. Jeder
Plan muss beantworten, **was den dritten Abdrift verhindert** — sonst ist er
nicht fertig.

## Auftrag

`docs/analysis/desktop-userspace-plan.md` gegen den heutigen Stand pruefen
(er ist vom 2026-09-01, seitdem sind DESK01–DESK42 gelaufen), dann in
Arbeitseinheiten schneiden, die einzeln abnehmbar sind. Keine Umsetzung, bevor
der Schnitt steht und der Anwender ihn gesehen hat.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.

## Fortschritt

| Einheit | Stand |
|---|---|
| U1 Instrument | **fertig** 2026-09-26 (`c141be2`) — Driftprobe 47, 12 Abweichungen gelistet |
| ACC10 (Vorbedingung) | **fertig** 2026-09-26 (`814de2c`) |
| U2 Zustandskanal | **fertig** 2026-09-26 (`d79e71f`) — Beweispunkt 1 des Plans dabei **widerlegt** |
| U3 Desktop endet (G1) | **fertig** 2026-09-26 — Driftprobe **47 → 38**; HW-Punkte 1+2 offen |
| U4 Kernel loest sich | **fertig** 2026-09-26 — Driftprobe **38 → 32**; zwei Stehenbleiber mit Messbegruendung |
| — Trennlinie — | *alles darueber ohne EL0 und ohne neue Werkzeugkette* |
| U5a Instrumente | **fertig** 2026-10-14 (`4ae60fae`) — Driftprobe v2, Konstantenabgleich, Zitat-Liste, Sequenz-Orakel |
| U6a Rust-EL0-Spike | **fertig** 2026-10-14 (`1257b7cc`) — HELLOGEM.NAP, Lader unveraendert |
| U5b Typen/Konstanten | **fertig** 2026-10-14 (`ac521aeb`) — Driftprobe 31 → 19 |
| U7 Zellmasse | **fertig** 2026-10-14 (`2c7895db`) — Driftprobe 19 → 11, 0 Pixel |
| U8a Entkopplung | **fertig** 2026-10-14 (`87adb580`) — Driftprobe 11 → 4, Kernel meldet „desktop up“ |
| U6b … U10 | Neuschnitt 2026-10-14, siehe unten |

## Hardware-Abnahme U3/U4 (2026-09-28)

Kern `92ec0ab7`. Anwender: QED per Doppelklick starten und beenden, TeraDesk,
QED **aus TeraDesk heraus**, Fensterrekonstruktion — *„funktionierte schon
laengst"* bzw. am 2026-09-26 bestaetigt. **Alt+Buchstabe: funktioniert** —
damit ist U4s `Keytbl`-Weg (`svc #14` statt `crate::keymap`) auf echter
Hardware belegt, und zwar der Klickweg davor mit.

**Damit sind U1–U4 vollstaendig abgenommen.** Offen bleiben nur U5–U10
(nach dem Termin) und [X03](X03.md) davor.

## Neuschnitt U5–U10 (Pruefung 2026-10-14, Anwender-Freigabe 2026-10-14)

Pruefung gegen den heutigen Code: `docs/analysis/desk43-u5-u10-review-2026-10-14.md`
(gegen Plan v2, `docs/analysis/desktop-userspace-plan-v2.md` §5).

**Ueberholt in diesem Ticket:** die „rund 40 Aufrufe am Trap vorbei“ (heute
keine; Driftprobe 31 = Typ-/Konstanten-/Makro-Importe) und der Absatz zu
D-R1-5/D-R1-6 (`ap_id = 0` und `ap_private` sind erledigt,
`tos-api/src/aes/appl.rs:200-224`, `aes/init.rs:61-65,98`;
`save_windows`/`restore_windows` ohne Produktivaufrufer).

**Freigegeben vom Anwender:**
- **D1** neuer Schnitt, seriell: U5a → U6a → U5b → U7 → U8a → U6b → U8b → U9 → U10;
  ein Quelltext, zwei Linkziele (Kernel-Bibliothek und `.NAP`) ab U8b.
- **D2** Werkzeugkette T1: Rust bleibt (*„ja keep rust“*). Rust-Bibliothek
  `desktop` + duennes Binaer-Crate, `no_std` + `alloc`, gelinkt mit
  `sdk/crt0.S` + `sdk/newtos.ld` ueber `rust-lld`; kein C-Neuschrieb, kein
  `std`-Port. Die Laufzeit-Crate (Trap-Bindungen, `GlobalAlloc` ueber
  `Malloc`-Hunks, `panic_handler`) steht danach auch anderen Rust-Programmen
  offen.

**Empfehlungen, Entscheidung faellig vor der jeweiligen Einheit:** D3
eingebettet im Kernel-Image (U9), D4 Debug-Kanal im Produkt aus + Ersatz fuer
das Orakel `[desk] desktop up …` (U8a), D5 Rueckfall-Flag nur bis Pi-Abnahme
U9, D6 Absturzpolitik wie EmuTOS nach Analyse (U9), D7 PROC03 umformulieren,
D8 Ticket-/Plantexte (dieser Abschnitt).

**D4 entschieden (Anwender 2026-10-14):** Im Produkt keine Debug-Ausgaben des
Desktops (EmuTOS: `KDEBUG` auskompiliert). Die Orakelzeile
`[desk] desktop up …` wird durch eine Kernel-Meldung ersetzt, wenn das
Desktop-Programm sein erstes `evnt_multi` erreicht (gleiches Kriterium, von
aussen beobachtet). Die vier Desktop-Selbsttests (DESK45) werden ein eigenes
Testprogramm. Kein neuer EL0-Debug-Kanal.
