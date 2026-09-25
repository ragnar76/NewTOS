# DESK40 — Zwei fehlende `valid_drive`-Pruefungen

Status: **OFFEN** · Prioritaet: **P4** · Typ: Konformitaet — Nebenbefunde DESK31/DESK38

Abhaengigkeiten: [DESK38](DESK38.md) (erledigt — `valid_drive` gibt es seitdem)

## Die Stellen

1. **`check_alt_letter_key` ruft kein `valid_drive`**, zeigt also nie
   `STNODRIV`. EmuTOS filtert dort unmoegliche Buchstaben **vor** dem Aufruf
   heraus, mit eigener Begruendung im Quelltext — *„otherwise valid_drive()
   will issue an (ugly) error message"* (`deskmain.c:1147-1153`). Die
   Inline-Pruefung bleibt also; es fehlt nur der Aufruf danach.
2. **`fun_desk2desk` hat EmuTOS' Quell-Schalter nicht**
   (`deskfun.c:1284-1294`): `valid_drive` auf **Quelle und Ziel**, und „zwei
   Symbole, derselbe Laufwerksbuchstabe" → `illegal_op_msg`. Unser
   Selbstwurf-Test geht heute nur ueber die Objekt-Id, erkennt also zwei
   verschiedene Symbole desselben Laufwerks nicht.

## Arbeit

Beide Stellen nachziehen. `valid_drive` und `illegal_op_msg` gibt es seit
DESK38 bzw. DESK31 — es sind Aufrufe, keine neuen Funktionen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
