# REF02 — Die SD-Spezifikationen fehlen in `references/`, und die Hardwarenotiz nennt keine Zeitgrenzen

Status: **OFFEN** · Prioritaet: **P2** · Typ: Quellenlage (Researcher) — aus dem EMMC-Audit

Abhaengigkeiten: `docs/api/hw/emmc-audit.md` (Befund S1)

## Warum

Das EMMC-Audit hat einen Treiberfehler gefunden, der eine Zeitgrenze
verletzt — und dann festgestellt, dass diese Zeitgrenze **nirgends bei uns
aufgeschrieben ist**:

* Die SD-Spezifikationen (Physical Layer Simplified, Host Controller
  Simplified) liegen **nicht** in `references/`, anders als EmuTOS und
  FreeMiNT.
* `docs/hardware/emmc-sd.md` nennt **keinerlei** Zeitgrenzen.

**Der Treiber konnte sie nicht einhalten, weil sie nicht da waren.** Das ist
genau der Fall, den `docs/SOURCE-ANALYSIS-PROTOCOL.md` Track B verhindern
soll, und er hat uns die FS04-Sekunden gekostet.

## Auftrag

1. Die beiden SD-Spezifikationen nach `references/` holen und **pinnen**, wie
   EmuTOS und FreeMiNT — mit Fassungsnummer, damit ein Zitat spaeter
   nachpruefbar bleibt. Lizenz- und Weitergabefrage dabei klaeren und
   festhalten (die „Simplified Specifications" sind dafuer der uebliche Weg).
2. `docs/hardware/emmc-sd.md` um die Zeitgrenzen ergaenzen, je mit
   Abschnittsangabe: Programmierzeit der Karte, Busy nach dem letzten Block,
   Lese-Zeitgrenze, die Empfehlung an den Host. Das Audit hat sie bereits
   zusammengetragen — von dort uebernehmen **und gegen die Quelle pruefen**.
3. **Zwei Korrekturen an `docs/hardware/emmc-sd.md`**, vom Audit gemeldet:
   * §3 gibt fuer ACMD41 `0x5100_0000` an — **ohne Spannungsfenster**. Der
     Treiber macht es richtig; die Notiz ist falsch.
   * Die Behauptung, `0xFE300000` sei „sdhost → Wi-Fi", ist ungeprueft. Fuer
     uns folgenlos, weil wir beide Basen probieren — aber eine ungepruefte
     Behauptung in einer Hardwarenotiz ist eine Falle fuer den naechsten.

## Nachtrag 2026-09-27 aus HW03

HW03 brauchte ein Zitat je R1-Fehlerbit und stand vor derselben Luecke. Der
Entwickler hat sich die **Physical Layer Simplified Specification Ver. 9.10**
von der oeffentlichen Downloadseite der SD Association geholt und unter
`references/sd-spec/` abgelegt — das Verzeichnis ist **gitignored**
(`/references/*`), die Fassung ist also **nicht** gepinnt und liegt nur auf
diesem Rechner.

**Zusaetzliche Aufgabe fuer dieses Ticket:** das Audit
(`docs/api/hw/emmc-audit.md`) zitiert **Ver. 6.00**, HW03s Bitpositionen
stammen aus **9.10**, und die beiden sind **nicht gegeneinander geprueft**.
Beim Pinnen ist also zu entscheiden, welche Fassung die Referenz ist, und die
Zitate der anderen sind nachzuziehen.

## Abnahme

`references/` traegt die gepinnten Spezifikationen; `docs/hardware/emmc-sd.md`
traegt die Zeitgrenzen mit Abschnittsangabe und ist um die beiden Fehler
bereinigt. Keine Codeaenderung.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
