# DESK42 — Der `Show info`-Kasten sieht EmuTOS' nicht aehnlich

Status: **OFFEN** · Prioritaet: **P3** · Typ: Konformitaet — Pi-Runde 2026-09-25

Abhaengigkeiten: [DESK38](DESK38.md) (erledigt)

## Sachverhalt

DESK38 hat `Show info` gebaut — es **funktioniert** (auf der Pi geprueft:
Laufwerk, Papierkorb, Drucker, Verknuepfung). Der Anwender: *„funktioniert,
aber das Fenster hat mit dem von EmuTOS rein gar nichts gemeinsam."*

Es geht also nicht um das Verhalten, sondern um den **Baum**: EmuTOS' Kaesten
sind Ressourcenbaeume mit fester Anordnung — `ADDISKIN` (Laufwerk),
`ADFILEIN` (Datei/Ordner) und die uebrigen in `references/emutos/desk/desk_rsc.c`.
DESK38 hat sie funktional nachgebaut, nicht in ihrer Form.

## Arbeit

Die betroffenen Baeume aus `desk_rsc.c` auslesen und Objekt fuer Objekt
nachbauen — dieselbe Reihenfolge, dieselben Objektnummern, dieselben
Beschriftungen, wie es DESK03 fuer `ADINSDSK` und DESK05 fuer `ADDESKCF`
gemacht haben. Die 3D-Rollen nach der Tafel in `inf.rs` (DESK01b); ein
3D-Objekt waechst vor dem Zeichnen um 2 px je Seite, also EmuTOS' eigene
Masse nehmen statt zu draengen.

Abnahme: Screendump je Kasten gegen den entsprechenden Baum. Wo eine
Abweichung bleibt (etwa weil eine Zeile bei uns keinen Inhalt hat), wird sie
benannt statt stillschweigend gelassen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
