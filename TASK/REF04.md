# REF04 — Die woertliche Tabelle 4-42 in `emmc-sd.md` gegen die Lizenzbedingungen

Status: **OFFEN — Anwenderentscheidung** · Prioritaet: **P2** · Typ: Lizenz / Veroeffentlichung — aus REF02

Abhaengigkeiten: [REF02](REF02.md) (dort im Wortlaut gelesen)

## Der Sachverhalt

`docs/hardware/emmc-sd.md` §5 (aus [HW03](HW03.md)) gibt die **komplette
Description-Spalte der Tabelle 4-42** der SD Physical Layer Simplified
Specification wieder — **15 Zeilen Originaltext**.

Die Lizenzbedingungen der SD Association erlauben Nicht-Mitgliedern
(§2) Herunterladen, Ansehen und Drucken fuer den internen Gebrauch, verbieten
aber ausdruecklich:

> *„(a) copy portions of the Simplified Specs (whether by screen scraping,
> screen shots, electronically or otherwise) … or (c) distribute the Simplified
> Specs or any portion thereof."*

§3 — Auszuege in eigene Dokumentation zu uebernehmen — gilt **nur
SDA-Mitgliedern**. Im PDF steht zusaetzlich: *„Distribution of the Simplified
Specification, other than through the Site, is a violation of the Terms."*

**Das Repo ist oeffentlich.**

## Warum es hier steht statt behoben zu sein

Der Inhalt ist **fachlich richtig** und war HW03s Arbeit — es geht nicht um
einen Fehler, sondern um die Form. Und es ist eine Entscheidung ueber die
Veroeffentlichung des eigenen Projekts, also eine des Anwenders, nicht eine,
die ein Agent nebenbei trifft.

Die Lesart stammt aus REF02 und ist **keine Rechtsberatung**.

## Die drei Wege

1. **In eigene Worte fassen.** Jedes Bit behaelt Nummer, Name und unsere
   Beschreibung dessen, was es fuer den Treiber bedeutet; der Originalwortlaut
   entfaellt, die Abschnittsangabe bleibt als Verweis. Sachlich verlustfrei,
   weil unsere Leser den Treiber verstehen muessen, nicht die Spec
   nachschlagen. **Empfehlung.**
2. **Kuerzen** auf Bitnummer + Name + Typspalte (die blosse Zuordnung, die man
   ohnehin aus dem Code liest) und fuer den Rest auf §4.10.1 verweisen.
3. **So lassen** — dann als bewusste Entscheidung mit Begruendung in der Datei
   vermerken, nicht stillschweigend.

## Abnahme

`emmc-sd.md` §5 traegt die gewaehlte Form, und `references/README.md` haelt
fest, was fuer kuenftige Uebernahmen aus dieser Quelle gilt — damit die naechste
Einheit nicht wieder hineinlaeuft.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
