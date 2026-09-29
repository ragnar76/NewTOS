# SND02 — HDMI-Ton: die Quellenlage ist ungeklaert, nicht guenstig

Status: **OFFEN** · Prioritaet: **P3** (nach dem OFAM-Termin) · Typ: Track-B-Recherche — aus [SND01](SND01.md) Schritt 2

Abhaengigkeiten: [SND01](SND01.md) (Klinke zuerst; HDMI ist der zweite Ausgang)

## Warum dieses Ticket existiert

Der Anwender will den Ton **ueber HDMI** (2026-09-28), Klinke als Rueckfall.
Der Lead hat daraufhin aus dem Geraetebaum geschlossen, HDMI-Ton sei „ein
normaler Treiber gegen dokumentierte Register statt eines Firmware-Handels".

**Das ist widerrufen.** Track B hat es geprueft: das **BCM2711-Datenblatt
(Rel. 4) hat gar kein HDMI-Kapitel** — dreizehn Kapitel, keines davon HDMI.

## Was der Geraetebaum zeigt und was er nicht zeigt

**Zeigt:** der HDMI-Knoten hat einen eigenen Audiotakt, einen eigenen
DMA-Kanal und benannte Registerbereiche (`hdmi`, `packet`, `metadata`, `hd`).

**Zeigt nicht:** ob diese Register irgendwo **dokumentiert** sind. Ein
Geraetebaum ordnet Adressen und Namen zu; er ist keine Registerbeschreibung.
Der Lead hat aus dem einen auf das andere geschlossen — das war der Fehler.

Zwei Beobachtungen, die eher in die andere Richtung deuten und **nicht**
weiterverfolgt wurden: der DMA-Name des Knotens ist `audio-**rx**` (nicht
`tx`), und die Bootkarte traegt `snd_bcm2835.enable_hdmi` in ihren Schaltern —
also einen **Firmware**-Schalter.

## Auftrag

1. **Feststellen, ob es ueberhaupt eine Primaerquelle gibt.** Raspberry-Pi-
   Dokumentation, VideoCore-Unterlagen, alles was sich pinnen laesst. Gibt es
   keine, ist das die Antwort — und dann ist HDMI-Ton bei uns eine
   Reverse-Engineering-Aufgabe und keine Treiberaufgabe.
2. **Den Firmware-Weg pruefen.** `snd_bcm2835` und der `enable_hdmi`-Schalter
   deuten darauf, dass der Ton normalerweise durch die Firmware geht. Ob es
   dafuer eine Mailbox-Schnittstelle gibt, die wir bedienen koennen, ist
   ungeklaert — und waere der weit billigere Weg.
3. Erst danach entscheiden, ob HDMI-Ton fuer NewTOS realistisch ist.

**Nicht anfangen zu bauen, bevor Punkt 1 beantwortet ist** (Regel 6: keine
erfundene Hardware).

## Abnahme

Eine Antwort auf „gibt es eine Quelle" — ja mit Pin, oder nein mit Begruendung.
Kein Code.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
