# SND01 — Schritt 1 (Quellenanalyse) abgeschlossen

**Datum:** 2026-09-29 · **Art:** Spec, kein Code · **Ticket:** `TASK/SND01.md`,
Teil A / Schritt 1 der dortigen Reihenfolge.

## Ergebnis

`docs/api/xbios/_dmasound-group.md` — Vollspec der XBIOS-DMA-Sound-Gruppe
**0x80…0x8D** (14 Funktionen), extrahiert nach
`docs/SOURCE-ANALYSIS-PROTOCOL.md` Track A aus `references/emutos` @
`1a7484ab`, ergänzt um `references/freemint` @ `f906416a` dort, wo EmuTOS
die Register zwar programmiert, aber nie benutzt (Puffer- und
Interruptablauf).

Damit ist CLAUDE.md Regel 5 („Spec vor Code") für diese Gruppe erfüllt.

## Was damit beantwortet ist

* Welche Funktionen EmuTOS führt und unter welchem Gatter (§1) —
  `CONF_WITH_DMASOUND` und `CONF_WITH_XBIOS_SOUND`, im `make 512`-Bau
  beide `1`, beide laufzeit-torgesteuert.
* Der Puffermechanismus vollständig (§4): Übergabe, exklusive
  Endadresse, Schleife, Interrupt- bzw. Polling-Vertrag, die
  Bit-7-Bankumschaltung, Raten, Sampleformat.
* Die Linie Vertrag/Interna nach `docs/COMPATIBILITY.md` (§8).
* Die Lücken, die als dokumentiertes „nicht vorhanden" zu führen sind
  (§9) — allen voran der Ende-des-Puffers-Interrupt.

## Was offen bleibt (Entscheidungen, keine Recherchelücken)

1. Zeigerbreite für `Setbuffer`/`Buffptr` unter LP64 (§9.2).
2. Bytefolge der 16-Bit-Abtastwerte (§9.3).
3. Umgang mit dem Timer-A-Interrupt: MFP-Ersatz oder GSXB-Weg (§9.1).
4. `LAST_ENTRY`-Nachführung in `docs/api/xbios/_dispatcher.md §4` (§10.4).
5. Sechs Abweichungskandidaten ohne D-Nummer (§10.6).

## Ausdrücklich nicht getan

* Kein Produktcode berührt — `tos-api/src/xbios/system.rs` und der
  Dispatcher bleiben unverändert; die nötigen Änderungen sind in der
  Spec beschrieben (§1.4, §10.3).
* `docs/STATUS.md` und `TASK/SND01.md` nicht angefasst.
* Teil B des Tickets (Track B: PWM/HDMI auf der Pi) ist **nicht**
  Gegenstand dieser Spec.
