# A42 — Quittung

**Status: ERLEDIGT · hosttestbelegt · in QEMU gegengemessen · Abnahme durch den
Tester offen** · 2026-09-13 · `wind_get(WF_NAME)` und `wind_get(WF_INFO)`
verweigern jetzt wie EmuTOS' `wm_get`, der Kommentar mit der gemlib-Begruendung
ist mitkorrigiert, und Fall 102 der m05-Tabelle stimmt nach dem Fix mit der
EmuTOS-Aufnahme ueberein. **Committet als `087ea43`** — der Baum war sauber.

## Quelle, selbst nachgeprueft

`references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`, Zielprofil
Falcon030/TOS4. Die Zeilennummern aus dem Ticket stimmen alle drei:

* `aes/gemwmlib.c:1591-1592` — `default:` / `return FALSE;`, der letzte Arm des
  zweiten `switch` in `wm_get`. Das `return` steht **vor** `if (which != -1)
  w_getsize(...)` und vor dem `return TRUE`, also wird kein `poutwds`-Wort
  geschrieben. Ein Zeiger des Aufrufers wird in `wm_get` ueberhaupt nirgends
  gelesen — die Signatur ist `BOOL wm_get(WORD, WORD, WORD *poutwds, WORD
  *pinwds)`, und `pinwds` wird nur von `WF_COLOR`/`WF_DCOLOR` als Gadgetnummer
  benutzt.
* `aes/gemwmlib.c:1660` — `case WF_NAME:` in `wm_set`.
* `aes/gemwmlib.c:1668` — `case WF_INFO:` in `wm_set`.

Der `switch` in `wm_get` (`:1527-1592`) hat Zweige fuer `WF_WXYWH`, `WF_CXYWH`,
`WF_PXYWH`, `WF_FXYWH`, `WF_HSLIDE`, `WF_VSLIDE`, `WF_HSLSIZ`, `WF_VSLSIZ`,
`WF_TOP`, `WF_FIRSTXYWH`, `WF_NEXTXYWH`, `WF_SCREEN`, `WF_COLOR`, `WF_DCOLOR`,
`WF_OWNER` — und fuer nichts sonst. `WF_KIND` (1), `WF_NAME` (2), `WF_INFO` (3)
und `WF_NEWDESK` (14) fallen in den Default-Arm.

## Der Kern — was daran lehrreich ist

Der Defekt war nicht Unachtsamkeit, er hatte eine **Begruendung im Kommentar**:

```
// WF_NAME/WF_INFO get: copy the stored title/info out to the
// caller's buffer (pointer at `&int_in[2]`, caller width —
// gemlib `wind_get`, `a_wind_get.c:344`; ...). Per the gemlib
// contract, an empty title returns FALSE and leaves the caller's
// buffer untouched.
```

gemlib ist die **Client-Bindung**. Sie darf jeden Aufruf deklarieren, den sie
will; was ein Aufruf *tut*, entscheidet die AES auf der anderen Seite des Traps.
Ein Programm, das gegen `a_wind_get.c` gebaut ist und auf einem TT laeuft,
bekommt `FALSE` und einen unberuehrten Puffer — der Header verspricht etwas, das
niemand einloest. Wir sind der falschen Autoritaet gefolgt und haben die
Abweichung mit ihr belegt.

Deshalb ist der Kommentar **mitkorrigiert** und nicht nur der Code entfernt: ein
falscher Kommentar ueberlebt falschen Code. Die neue Fassung steht im
Default-Arm von `wind_get` und benennt beides — dass `wm_get` die Felder nicht
kennt, und warum die gemlib-Deklaration kein Beleg ist.

## Was ich geaendert habe

**`tos-api/src/aes/window.rs`**

* Der `WF_NAME | WF_INFO`-Arm in `wind_get` ist entfallen. Beide Felder fallen
  jetzt in den vorhandenen `_`-Arm: `pb.set_ret(0)`, kein `set_intout`, kein
  Schreiben durch einen Zeiger.
* Der Kommentar am `_`-Arm fuehrt jetzt alle drei verweigerten Felder
  (`WF_KIND`, `WF_NAME`, `WF_INFO`), nennt die EmuTOS-Fundstellen und schreibt
  die gemlib-Sache ausdruecklich als das hin, was sie war: die falsche
  Autoritaet, mit der Messung als Beleg.
* **`copy_out_cstr` geloescht** — der Helfer hatte genau diesen einen Aufrufer.
  Stehengelassen waere er eine Warnung (`dead_code`) und eine Einladung.
  `copy_cstr` (die Set-Richtung) und `name_len` bleiben, beide haben weitere
  Aufrufer.
* `wf_str_ptr`s Dokumentation redet nicht mehr von `wind_get`. Sie nennt jetzt
  die `wind_set`-Felder (`WF_NAME`/`WF_INFO`/`WF_NEWDESK` — letzteres holt sich
  seinen Baumzeiger in `set_newdesk` ueber dieselbe Funktion) und sagt in einem
  Satz, warum `wind_get` sie nicht braucht.

**`tos-api/src/aes/mod.rs`** — zwei Dokumentationskommentare an `Aespb::ain_len`
und `Aespb::intin_ptr` fuehrten `wind_get` als Nutzer der beiden
Zeigerkonventionen. Nach dem Fix ist das falsch; beide nennen jetzt nur noch
`wind_set`. Kein Code.

**`docs/api/aes/window-graphics.md`** — die Spezifikation war **nicht falsch**
(die `wind_get`-Feldtabelle listet `WF_NAME`/`WF_INFO` gar nicht, und darunter
steht „Any unlisted field → `default: return FALSE`"), aber sie war stumm: die
Feldnummerntabelle nennt `WF_NAME` „title string ptr" ohne Hinweis, dass nur
`wind_set` das kann — genau die Luecke, in die die gemlib-Begruendung passte.
Neu: eine Fussnote ‖ an `WF_NAME`/`WF_INFO` nach dem Muster der vorhandenen ‡ an
`WF_NEWDESK`, und ein Absatz unter „Fields with no handler (return `FALSE`)",
der die drei `wind_set`-only-Felder zusammen nennt.

**`tos-api/tests/aes_window_name_get.rs`** — neu, 3 Faelle.

## Test vor Fix — die rote Ausgabe

Der Test prueft **den Puffer zuerst** und den Rueckgabewert danach. Das ist
Absicht: „gibt 0 zurueck, schreibt aber trotzdem" waere weiterhin falsch, und
ein Test, der am Rueckgabewert stehenbleibt, faengt das nicht. Der Puffer traegt
vor dem Aufruf 32 Byte `0x5A` und wird danach byteweise verglichen.

```
running 3 tests
test wf_name_is_refused_and_leaves_the_callers_buffer_untouched ... FAILED
test the_refusal_writes_no_words_and_stays_inside_the_title_fields ... ok
test wf_info_is_refused_and_leaves_the_callers_buffer_untouched ... FAILED

---- wf_name_is_refused_and_leaves_the_callers_buffer_untouched stdout ----
wind_get(WF_NAME) via addr_in[0]: the caller's buffer was written at byte 0
(0x41, expected 0x5A). ... Buffer: [41, 42, 43, 44, 45, 46, 47, 48, 00, 5A, ...]

---- wf_info_is_refused_and_leaves_the_callers_buffer_untouched stdout ----
wind_get(WF_INFO) via addr_in[0]: the caller's buffer was written at byte 0
(0x33, expected 0x5A). ... Buffer: [33, 20, 69, 74, 65, 6D, 73, 2C, 20, 31, ...]

test result: FAILED. 1 passed; 2 failed; 0 ignored
```

`41 42 43 …` ist „ABCDEFGH", `33 20 69 74 …` ist „3 items, 12 KB" — der
gespeicherte Titel bzw. die Infozeile, in den Puffer des Aufrufers kopiert.

Eine erste Fassung pruefte den Rueckgabewert zuerst; die scheiterte an
`left: 1, right: 0`, was den halben Defekt zeigt. Die Reihenfolge ist getauscht,
damit die rote Ausgabe die Stelle nennt, die wirklich weh tut.

Nach dem Fix:

```
running 3 tests
test the_refusal_writes_no_words_and_stays_inside_the_title_fields ... ok
test wf_info_is_refused_and_leaves_the_callers_buffer_untouched ... ok
test wf_name_is_refused_and_leaves_the_callers_buffer_untouched ... ok

test result: ok. 3 passed; 0 failed; 0 ignored
```

Der dritte Fall war schon vorher gruen und ist ein **Waechter**, kein Beleg: er
faehrt `OUT_LEN 5` mit `addr_in[0] = 0`, wo der alte Code an seiner
Null-Zeiger-Pruefung ohnehin `0` lieferte. Er haelt jetzt fest, dass die
Verweigerung keine `int_out`-Woerter schreibt (`int_out[1..5]` = 0, `int_out[5]`
noch vergiftet), dass `WF_KIND` weiterhin verweigert wird und dass `WF_WXYWH`
nebenan unveraendert antwortet — die Reparatur ist nicht breiter geworden als
die zwei Felder.

Beide Aufrufkonventionen sind abgedeckt: der Zeiger in `addr_in[0]` mit
`AIN_LEN 1` (so misst die m05-Tabelle) **und** der in `int_in[2..]` gepackte mit
`AIN_LEN 0` (so ruft gemlib). Die AES verweigert das Feld, nicht die
Konvention.

## Gegenmessung — Fall 102 stimmt jetzt

QEMU `raspi4b`, headless, 640x480 (TT-Profil), ohne Karte, `#Z`-Autorun; Rezept
und Skripte aus `measurements/measurement-2026-09-13-regression/method/` unveraendert
uebernommen. Alle Kennzahlen in
[measurements/measurement-2026-09-13-a42/RUNINFO.TXT](../measurements/measurement-2026-09-13-a42/RUNINFO.TXT).

| | EmuTOS 1.4 (TT/Hatari) | NewTOS vorher | NewTOS nachher |
|---|---|---|---|
| Fall 102 `int_out[0]` | `0` | `1` | **`0`** |

* `aes-m05-addrin/CLASSIFY-vs-emutos.txt`: **„case-blocks with a NON-noise
  field: 0"** (vorher 1). Uebrig bleiben in allen fuenf Faellen genau die sechs
  bekannten `AESGLOBAL`-Rauschfelder `global[2]/[3]/[4]/[10]/[11]/[12]`
  (ap_id, ap_private-Halbwoerter, ap_planes 16 gegen 4, Kerneladresse,
  ap_3resv-Stapelrest) — dieselben, die auch die drei unbeteiligten Faelle 88,
  93 und 94 zeigen.
* `aes-m05-addrin/DIFF-vs-prefix-newtos.txt`: gegen den Vorher-Lauf aendert sich
  in der ganzen Tabelle **genau ein Wort**. Kein anderer Fall bewegt sich.
* Fall 109 (`wind_get WF_NAME` ohne gesetzten Titel) stimmte schon vorher: der
  alte Code lieferte dort ueber seine Leerstring-Pruefung zufaellig das richtige
  Ergebnis aus dem falschen Grund.

**Was der Lauf nicht zeigt:** der 120-Byte-AES-Datensatz hat keinen Platz fuer
den Inhalt des Puffers (steht so im Kopf der Falltabelle). Dass der Puffer
unberuehrt bleibt, belegt der Hosttest byteweise, nicht diese Messung.

## Belege

* `cargo run -p xtask -- test`: **502 gruen / 0 rot / 4 ignoriert**
  (Sollstand 499 + meine 3 neuen Faelle). `cargo test -p tos-api` allein:
  **492 / 0 / 4** (Sollstand 489 + 3).
* Kein bestehender Test musste geaendert werden, und **kein Eintrag der
  `SANCTIONED`-Liste** in `tos-api/tests/aes_reference/harness.rs` war
  betroffen: sie hat genau einen Eintrag (`aes-r1.tsv` Fall 166, `WF_SCREEN`),
  und die Referenztabellen `aes-r1.tsv`/`aes-r2-wind.tsv` enthalten keinen
  `WF_NAME`/`WF_INFO`-Lesefall. Die m05-Tabelle ist keine Rust-Testquelle.
* `cargo xtask build --board rpi4 --release` und `--board qemu-virt --release`,
  beide Exit 0, je 0 Fehler, `tos-api` 64 + `kernel` 94 Warnungen — **identisch
  zur Grundlinie** in `measurements/measurement-2026-09-13-regression/build-*.log`.
  **Echter Neubau nachgewiesen:** vor jedem der beiden Laeufe wurde
  `tos-api/src/aes/window.rs` (und `mod.rs`) angefasst, damit das geteilte
  Zielverzeichnis nicht „up to date" melden kann; beide Protokolle zeigen
  `Compiling tos-api` **und** `Compiling kernel`, und die erzeugten ELF sind
  verschieden (`43dbbb2b…` rpi4 gegen `3e348c00…` qemu-virt). Der rpi4-Lauf ist
  reproduzierbar: zweimal gebaut, beide Male `43dbbb2b…`.
* Clippy: `-p tos-api --lib` **81** Warnungen, `-p kernel --lib` je Board
  **125** — beide exakt die Grundlinienzahl. Der Warnungstext ist gegen die
  Grundlinie **zeichengleich**; im `--> `-Vergleich unterscheiden sich nur zehn
  Zeilennummern in `window.rs`, weil die Datei kuerzer geworden ist (12 Zeilen
  vor `wind_get` durch den geloeschten Helfer, 19 danach). Null neue
  Warnungen.
* Rauchprobe: der Messlauf selbst ist eine — QEMU `raspi4b` Release bootet,
  faehrt `C:\AUTO\HELLO.NCP`, startet den Dumper ueber `#Z`, faehrt alle 5
  Faelle, schreibt AES.BIN/AES.LOG und landet danach im Desktop
  (`[desk] desktop up (1 drives)`), ohne Panik im seriellen Mitschnitt
  (`aes-m05-addrin/BOOT-SERIAL.txt`).

## Weitere gemlib-statt-EmuTOS-Zweige in `wind_get`: keine

Auftragsgemaess durchgegangen. Nach dem Fix beantwortet unser `wind_get`
**genau** die Felder, die `wm_get`s `switch` kennt — Feld fuer Feld verglichen,
kein Ueberschuss, kein Fehlender. `WF_SCREEN` (17) ist die einzige verbleibende
Abweichung und keine dieser Art: sie ist bewusst, registriert
(`docs/COMPATIBILITY.md`, `TASK/A54.md`, `SANCTIONED`-Eintrag) und geht in die
andere Richtung (wir verweigern, wo EmuTOS antwortet), weil der Puffer bei uns
nicht existiert und im EL1-Heap auch nicht existieren koennte.

Nicht geaendert, weil eigene Tickets: die `wind_set`-Seite habe ich bei der
Gelegenheit nur ueberflogen, nicht systematisch gegen `wm_set` gestellt — das
waere ein eigener Auftrag und steht hier ausdruecklich als **nicht geprueft**,
nicht als „sauber".

## Was ich nicht angefasst habe

* `docs/api/aes/_full-function-inventory.md` — die Befundtabelle II.6 ist eine
  **Momentaufnahme des Audits**, kein Zustandsbericht: ihre Zeile A-6
  (`WF_SCREEN`) beschreibt bis heute das Verhalten von **vor** A54. Ich habe die
  Konvention nicht gebrochen und A-1 nicht einseitig umgeschrieben; der
  Reparaturstand steht in dieser Quittung und in `docs/STATUS.md`.
* `tos-api/src/aes/window.rs` sonst — `wind_set`, die Rechteckliste, die
  Rahmenzeichnung. Der Fix ist ein entfernter `match`-Arm und ein geloeschter
  Helfer.
* Vorbestehende Warnungen in beiden Crates.

## Offen

1. **Abnahme durch den Tester** — Einzelheiten in der Uebergabe des
   Entwicklerberichts: QEMU-`raspi4b`-Boot beider Boards, Screendumps
   (Fenstertitel muessen unveraendert gezeichnet werden — die `wind_set`-Seite
   ist die, die den Titel speichert, und sie ist unberuehrt), und als
   Regressionsprobe die Tabellen `aes-r2-wind` und `aes-r3-wind-state`, die den
   Fenstermanager abdecken.
2. **Echte Pi 4**: nichts, was diese Aenderung spezifisch braucht. Sie ist rein
   logisch, ohne Hardwarebezug; der QEMU-Lauf ist der gleiche Code.
