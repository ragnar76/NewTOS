# PORT01 — Eine auf QED gezogene Datei erreicht die Anwendung nicht

Status: **OFFEN** · Prioritaet: **P2** · Typ: Belegter Defekt — Pi-Runde 2026-09-24

Abhaengigkeiten: [DESK19](DESK19.md) (erledigt)

## Sachverhalt

Auf der echten Pi 4: eine `.TXT`-Datei auf `QED.NAP` gezogen **startet** QED,
aber die Datei wird nicht geoeffnet — die Anwendung sieht ihr Argument nicht.

## Was schon ausgeschlossen ist

**Der Kernel uebergibt den Schwanz.** Beide Startwege in
`kernel/src/desk/main.rs::launch_native` gehen durch
`crate::launch::run_program_with_tail`: der GEM-Zweig ueber `run_gem_app`
(`:1072-1081`) und der Konsolenzweig direkt (`:1013`). Es gibt keinen Pfad,
der den Schwanz fuer `.NAP` verwirft.

**Und er kommt nachweislich an — bei einem anderen Programm.** DESK19s
Messung (`measurement-2026-09-23-desk19-desk20/`): dieselbe Geste auf
`CMDLINE.NCP` druckt

```
CMDLINE: len 13 arg "C:\README.TXT"     (Parameter = Full path)
CMDLINE: len 10 arg "README.TXT"        (Parameter = File name)
```

Der Unterschied zwischen den beiden Faellen ist **nicht** der Kernel, sondern
was die Anwendung mit der Basepage macht: `sdk/examples/cmdline.c` liest
`p_cmdlin` selbst, QED geht durch die SDK-Startroutine und erwartet
`argc`/`argv`.

## Wo zu suchen ist (Eingaben, keine Diagnose)

1. **Die Startroutine des SDK** — baut sie `argc`/`argv` aus `p_cmdlin`?
   `sdk/libc/`s crt0. Es gibt einen Praezedenzfall: ein frueherer
   QED-Startabsturz lag genau dort (crt0 `argc`/`argv`), nicht an der Stelle,
   die zuerst verdaechtigt wurde.
2. **Die Basepage-Struktur unter LP64.** Bekanntes, bisher **latentes**
   Problem: mintlibs TOS-Strukturen (`BASEPAGE`, `DTA`) sind auf aarch64
   8-Byte-Felder, auf dem Draht 4 Byte. Wenn die Startroutine `p_cmdlin` ueber
   die mintlib-Struktur liest statt ueber den Draht-Offset, liest sie
   danebenвЂ" und das waere genau dieses Symptom.
3. ~~QED selbst~~ — **ausgeschlossen.** Der Benutzer arbeitet auf seinem
   echten TT genau so: Datei auf QED ziehen, und zwar auf eine
   **Desktop-Verknuepfung**, auf die er sie ablegt. QED wertet den Dateinamen
   also aus, und der Weg ist sein normaler Arbeitsablauf, kein Sonderfall.

## Beide Ziehwege sind auf unserer Seite verdrahtet

Auch der zweite, den der Benutzer auf dem TT benutzt: das Ablegen auf eine
**Desktop-Verknuepfung** baut den Schwanz genauso
(`kernel/src/desk/dir.rs:1885`, `AType::File`-Zweig, seit DESK30) wie das
Ablegen im Fenster (`:1793`, `fun_win2win`, seit DESK19). Beide enden in
`do_aopen_drop`. Es gibt also keinen dritten Pfad, an dem der Schwanz
verlorengehen koennte — und wenn der Defekt in der Werkzeugkette liegt,
betrifft er **beide** Wege gleichermassen.

## Arbeitsregel

Die portierte Anwendung bleibt **unveraendert**: Korrekturen gehoeren in die
Werkzeugkette (`sdk/`), nicht in QEDs Quelltext. Einzige Ausnahme waere
m68k-Assembler, den es hier nicht gibt.

Abnahme: dieselbe Geste auf der Pi, QED oeffnet die Datei. Ein
Zwischenbeleg, der ohne Hardware geht: ein `.NAP`, das sein `argv` druckt —
`sdk/examples/cmdline.c` ist die Konsolenfassung davon und kann als Vorlage
dienen.

[Gemeinsame Regeln](README.md) gelten.
