# M06 — STATUS: DONE (2026-09-13)

Der Riegel entscheidet nach der **gemessenen** `ap_version`. `objc_sysvar` (48)
ist auf beiden Maschinen gefahren. Keine bestehende Tabelle liest sich anders,
keine bestehende CRC hat sich bewegt, kein bestehender Dump hat ein Byte
verloren.

## Wo der Riegel jetzt entscheidet — und warum dort

Die Frage ist an der einzigen Naht geteilt, an der sie teilbar ist:

* **Der Lader** (`td_aes_cases_parse`, Host wie Maschine, vor allem anderen)
  beantwortet weiter „ist dieser Opcode überhaupt absetzbar" — blockierend,
  fatal, unbelegbar, kein Opcode — und merkt sich **zusätzlich**, welche
  Fähigkeit die Tabelle braucht (`td_aes_case_table.features`). Das sind
  Eigenschaften der **Tabelle**, also ohne Maschine entscheidbar.
* **Die Maschine** (`td_aes_cases_version_refusal`, nach `appl_init`, vor dem
  ersten Fall **und** vor dem ersten Prelude) beantwortet „habe ich sie".
  `ap_version` schreibt `appl_init` (`aes/gemsuper.c:92`) und sonst nichts —
  früher gibt es die Größe nicht.

Verweigert wird die **ganze** Tabelle, nicht der einzelne Fall. Damit bleibt
die Form eines Dumps eine Eigenschaft allein seiner Tabelle: `count` Records
oder gar kein Dump. Genau darauf stützen sich die CRC-Bindung und `tosdiff`.

`td_run_aes` fragt dieselbe Funktion selbst, mit `global[0]`. Der Dumper fragt
sie vorher noch einmal, mit einem Fehlerobjekt — der eine Aufruf ist der
Bericht (Log nennt Fähigkeit und Zeile), der andere die Garantie, dass kein
späteres Programm das Fragen vergessen kann.

**Mengenzugehörigkeit, nicht `>=`.** `0x0340` ist die einzige Sprosse, die
`CONF_WITH_3D_OBJECTS` verlangt; `0x0330` und `0x0340` sind die beiden, die
`CONF_WITH_MENU_EXTENSION` verlangen (`config.h:1346-1349`). Die Implikation
läuft nur in eine Richtung, und eine Zahl, die die Leiter nie erzeugt
(`0x0331`, alles über `0x0340`), begründet nichts. Verweigern kostet dort eine
Tabelle, die vielleicht gelaufen wäre; Absetzen kostet eine hängende Maschine.

## Was geöffnet ist, und was nicht

* **Geöffnet:** `objc_sysvar` (48) und `menu_popup`/`attach`/`istart`/
  `settings` (36-39).
* **Nicht geöffnet:** die vier `CONF_WITH_PCGEM`-Opcodes, darunter
  `appl_yield` (17). Das Flag steht **nicht** in der `AES_VERSION`-Leiter, also
  kann keine gemessene Größe sie entscheiden. Sie bleiben verweigert, bis eine
  Primärquelle (Compendium/Profibuch) sagt, ob TOS 3.06 sie hat. Das Ticket
  führt `appl_yield` unter „was fehlt"; sein Riegelgrund war aber schon vorher
  `PCGEM` und nicht `GATED` — eine versionsgemessene Entscheidung erreicht ihn
  nicht.
* `CONF_WITH_WINDOW_COLOURS` und `CONF_WITH_GRAF_MOUSE_EXTENSION` entfernen
  **keinen** Opcode; sie sperren Felder und Modi innerhalb vorhandener Opcodes
  (M07).

## Der Nachweis

`tosdump/cases/aes-m06-sysvar.tsv` — 19 Fälle, 3 Preludes, CRC `458748F2`,
einmalig geschrieben. Auf **beiden** Maschinen gefahren
(`measurement-2026-09-13-m06-sysvar/`), je „all 19 cases ran", `--validate`
accepted.

**NewTOS' `objc_sysvar` stimmt in 18 von 19 Fällen wortgleich mit EmuTOS** —
alle sechs Get-Arme auf den `init_3d`-Vorgaben, beide Rückfallklassen je
Richtung, der `in1 == -1`-Arm und alle drei Rückleseproben. Der einzige
Unterschied ist der in der Tabelle **vorab deklarierte**: Fall 119
`set ACTBUTCOL 20` → EmuTOS `0`, NewTOS `1`, weil `ob_sysvar` gegen
`gl_ws.ws_ncolors` prüft und das 16 (TT, 4 Planes) gegen 256 (NewTOS,
`DEV_TAB[13]`) ist.

## Rückwärtskompatibilität, vierfach belegt

Zahlen: `measurement-2026-09-13-m06-sysvar/BACKCOMPAT.TXT`.

1. Alle **elf** Tabellen unter `tosdump/cases/` mit unveränderter CRC, gleichen
   Fall-/Prelude-/Spaltenzahlen; keine der sechs AES-Tabellen braucht etwas
   von der Maschine (`features == 0`).
2. Alle sechs AES-Tabellen (413 Fälle) liefern mit altem und neuem `core/`
   gegen dasselbe Testdouble **byteweise identische** Records.
3. Ein wiederholter `aes-r4`-Lauf ist auf **beiden** Maschinen **byteidentisch**
   zum M05-Wiederholungslauf, Header eingeschlossen. M06 verantwortet null
   Bytes.
4. `tosdiff --validate` fällt über alle 71 `*.BIN` im Baum dasselbe Urteil wie
   die Vor-M06-Fassung (67 akzeptiert, 4 `TERADESK-*TRACE` sind gar keine
   TDMP-Dumps und waren es nie).

## Was offen bleibt

* **`appl_yield` und die drei anderen PCGEM-Opcodes** bleiben unmessbar, bis
  eine Primärquelle entscheidet. Das ist kein Werkzeugproblem mehr.
* **M07 wird leichter, aber nicht erledigt.** Die Maschinenphase ist ein
  fertiger Ort für eine Entscheidung, die den Opcode allein nicht kennt — der
  Parameter-Riegel könnte dort andocken. Der harmlose `evnt_multi`-Arm und der
  `form_error(n > 63)`-Arm brauchen ihn aber nicht: beide sind aus der Zeile
  allein entscheidbar und gehören in den Lader, nicht hierher.
* **M08 unberührt.** `mkrusttests` braucht für eine `objc_sysvar`-Tabelle keine
  Änderung (der Emitter ist opcode-blind), aber eine generierte Rust-Suite
  hätte hier ein eigenes Problem: NewTOS' 3D-Variablen sind globale Statics,
  und der Rust-Harness läuft parallel. Wer diese Tabelle dauerhaft bewachen
  will, muss das zuerst lösen.
* **Der Record hat weiterhin keinen Platz für `int_out[2]`, wo EmuTOS nichts
  schreibt und NewTOS schreibt** (Get INDBUTCOL/ACTBUTCOL/BACKGRCOL). Die
  Tabelle weicht dem mit `OUT_LEN 2` aus, weil `OUT_LEN 3` dort den
  uninitialisierten `xif`-Stack messen würde. Eigene Frage, eigenes Instrument.
