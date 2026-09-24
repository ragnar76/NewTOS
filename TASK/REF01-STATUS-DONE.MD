# REF01 — Quittung

**Status: ERLEDIGT — Referenzkonfiguration festgeschrieben, keine Produktcode-Aenderung** · 2026-09-24

## Was getan wurde

`tools/emutos-config.sh` (neu, ausfuehrbar) geschrieben: erzeugt
`docs/api/_emutos-config.md` (neu) mechanisch aus dem gepinnten EmuTOS-Baum
(`1a7484aba1bfba23e2c0899e243b6d8177fe41df`), ohne ihn zu veraendern.
Zweimal laufen lassen ergibt eine byte-identische Datei (`diff` leer,
geprueft). `references/emutos/` unveraendert (nur `.DS_Store` untracked,
wie vor Sessionbeginn).

Das Skript macht mehr als reine `-dM`-Textsubstitution: zwei `CONF_*`-Makros
(`CONF_WITH_SCSI_DRIVER`, `CONF_WITH_XHDI`) sind als
`!CONF_WITH_EXTERNAL_DISK_DRIVER` definiert (`config.h:1685`, `:1692`);
reine Textsubstitution liesse `!0` statt `1` stehen. Jeder nicht-literale
Rest wird arithmetisch ausgewertet (gleiche Semantik wie ein C-`#if`).

## Punkt 4 — die Maschinenfrage (Kern des Tickets)

`make 512` baut **ein** ROM fuer TT, Falcon und (unter Hatari) ST/STe, kein
maschinenspezifisches Sonderbild. Mechanismus (gelesen, nicht angenommen):
`include/has.h:26-45` macht `HAS_X` entweder zu einer festen `0` (Feature
aus) oder zu einer **Laufzeit**variable `has_x` (Feature an). Im
TARGET_512-Profil sind `CONF_WITH_STE_SHIFTER`/`_TT_SHIFTER`/`_VIDEL`/
`_VME` alle `1` — alle vier Erkennungspfade sind mitkompiliert. Welcher
tatsaechlich zutrifft, entscheidet `detect_video()`
(`bios/machine.c:101-162`) per Bus-Error-Sondierung realer Register
(STe `0xffff820d`, TT-Palette `0xffff8400`, Falcon-`HHT` `0xffff8282`);
`fill_cookie_jar()` (`bios/machine.c:665-694`) leitet daraus `cookie_mch`
(`include/cookie.h:49-56`) ab — unbedingt beim Boot, vor jedem AES/Desktop-
Code (`bios/bios.c:246-248`, `:279-280`). **Folge:** eine `1`-Zeile heisst
"mitkompiliert fuer alle vier Maschinen", nicht "auf dem Falcon aktiv" —
letzteres ist eine Laufzeitaussage ueber `cookie_mch`/`has_*`, die diese
Tabelle nicht treffen kann. Dieser Absatz steht jetzt im Kopf der Tabelle,
mit der aus `include/has.h` mechanisch extrahierten Liste aller 18
derart laufzeitgesteuerten `CONF_WITH_*`. Gegengeprueft: keine der fuenf
Stichproben (unten) haengt an einem dieser 18; nur eine Stelle in
`desk/`/`aes/` fragt `cookie_mch`/`HAS_*` ab (`aes/gemgsxif.c:161`).

## Punkt 3 — ACC01 gegengeprueft, nicht uebernommen

Haelt: **kein** `#ifdef TARGET_512`-Block in `config.h`. Zwei unabhaengige
Belege: (a) keiner der 7 gefundenen `#ifdef TARGET_*`-Bloecke heisst
`TARGET_512`; (b) der Literalstring `TARGET_512` kommt im gesamten Baum
genau einmal vor — `Makefile:561`, die definierende Zeile, nie ein
`config.h`-Verbraucher. Alle 7 Bloecke gezaehlt und tabelliert:
`TARGET_192` (`config.h:210-394`, 57 abweichende Werte — ACC01 zitierte
selbst `210-401`, 7 Zeilen zu weit, hier korrigiert), `TARGET_256`
(`:402-448`, 14), `TARGET_CART` (`:469-554`, 26), `TARGET_FLOPPY`
(`:559-563`, 2), `TARGET_AMIGA_ROM` (`:568-570`, 48), `TARGET_AMIGA_FLOPPY`
(`:575-577`, 50), `TARGET_LISA_FLOPPY` (`:597-599`, 54 — letzte drei
fremde Maschinenfamilien, nicht Atari, der Vollstaendigkeit halber
mitgezaehlt). Die Abweichungen sind per echtem Praeprozessorlauf je Block
ermittelt, nicht per Text-Scan des Blocks — ein Text-Scan haette Kaskaden
durch einen nicht-`CONF_`-praefigierten Schalter uebersehen
(`TARGET_192` kippt `DETECT_NATIVE_FEATURES`, was `CONF_WITH_ARANYM`s
generischen Default zweitausend Zeilen entfernt aendert; beim Bauen der
Tabelle tatsaechlich aufgetreten).

## Fuenf Stichproben — alle halten

| Ticket | Flag | Handpruefung | Tabelle |
|---|---|---|---|
| DESK05 | `CONF_WITH_DESKTOP_CONFIG` | `1` | `1` |
| DESK15 | `CONF_WITH_WINDOW_ICONS` | `1` | `1` |
| DESK15 | `CONF_WITH_DESKTOP_SHORTCUTS` | `1` | `1` |
| DESK14 | `CONF_WITH_READ_INF` | an | `1` |
| ACC01 | `.ACC`-Lebenszyklus | an keinem Schalter | keine passende Zeile — bestaetigt |

DESK15s eigene Zeilenzitate (`config.h:322-324` fuer den `TARGET_192`-Nullwert,
`:1866-1868` fuer den generischen Default) nachgezaehlt — stimmen exakt.
Keine Diskrepanz an einem der fuenf frueheren Tickets.

## Nebenbefund (nicht in REF01 verlangt, aber gepruefte Randbedingung)

`make 512`s echter Compile traegt zusaetzlich `-DWITH_AES=1 -DWITH_CLI=1`
(`Makefile:267`); geprueft ob relevant fuer die Tabelle: nein — `config.h`
liest `WITH_AES`/`WITH_CLI` nirgends (Verbraucher nur
`bios/endrom.c:15,30`, `vdi/vdi_mouse.c:26,67`, Datei-Auswahl beim Link,
kein `CONF_*`-Makro). Die `-DTARGET_512`-allein-Begruendung haelt
vollstaendig.

## Pruefung

Skript zweimal ausgefuehrt, `diff` der beiden Ausgaben leer. 106
`CONF_*`-Makros (wie in REF01 vorgerechnet, nachvollzogen). Kein
Produktcode angefasst (`kernel/`, `tos-api/`, `sdk/` unveraendert).
`docs/STATUS.md` Recent-Decisions-Eintrag 2026-09-24 gesetzt;
`TASK/REF01.md` Status auf ERLEDIGT + `## Abschluss` ergaenzt; eine Zeile
in `docs/SOURCE-ANALYSIS-PROTOCOL.md` Schritt 3.7 verweist kuenftige
Extraktionen auf `docs/api/_emutos-config.md` statt auf `config.h`.

## Offen

Keins. Einzige Randnotiz: ACC01s eigenes `TARGET_192`-Zitat war um 7 Zeilen
ungenau (`:210-401` statt `:210-394`) — ACC01 hatte bereits mehrere
Off-by-N-Zitate in seinem eigenen Abschluss eingeraeumt; diese zusaetzliche
gehoert in dieselbe Kategorie und rechtfertigt kein eigenes Ticket.

## Nachtrag — zwei Koordinator-/User-Korrekturen waehrend der Bearbeitung

**Skript-Robustheit.** `sh tools/emutos-config.sh` bestand die
Determinismuspruefung aus dem falschen Grund (Exitcode 0 trotz
Syntaxfehler, `/bin/sh` = bash im POSIX-Modus, `<(...)` dort ungueltig).
Neue Schutzklausel ganz am Skriptanfang prueft `$SHELLOPTS` auf `posix`
und `$BASH_VERSION` auf leer; geprueft unter `sh`/`zsh` (Exitcode 1,
Ausgabedatei unangetastet) und `bash` (laeuft durch). Siehe
`TASK/REF01.md` Nachtrag 1 fuer die volle Herleitung.

**Maschinenfrage geschaerft + volle Klassifikation.** Der Vorbehalt
„keine Maschinen-Aussage" gilt nur fuer die 18 laufzeitgesteuerten
Hardware-Schalter (`include/has.h:2`), nicht fuer die ganze Tabelle —
mechanisch gegen die Arbeitsmenge gegengeprueft (Schritt 6c, Ueber-
schneidung null). Die flache 106-Zeilen-Tabelle wurde durch eine
Klassifikation ersetzt (User-Auftrag): Hardware/Fremdmaschine (65) und
GEMDOS/BIOS/Dateisystem/Build-Debug (13) nur benannt; die 28
Grafik/AES/VDI-Schalter einzeln geprueft — **21 vorhanden**
(`datei:zeile`), **7 Luecke**. Neun von 19 Koordinator-Vermutungen
waren tatsaechlich vorhanden (`CONF_WITH_SEARCH`, `CONF_WITH_GDOS`,
`CONF_WITH_COLOUR_ICONS`, `CONF_WITH_WINDOW_COLOURS`,
`CONF_WITH_EXTENDED_OBJECTS`, `CONF_WITH_NICELINES`,
`CONF_WITH_MENU_EXTENSION`, `CONF_WITH_GRAF_MOUSE_EXTENSION`,
`CONF_WITH_PCGEM`) — Korrektur, keine Bestaetigung. Die
Klassifikationsdaten liegen als Tabelle direkt im Skript
(`CLASSIFICATION_DATA`), jeden Lauf gegen die lebende Namensliste
gegengeprueft (Schritt 6b). Determinismus erneut mit zwei `bash`-Laeufen
bestaetigt (`diff` leer). Siehe `TASK/REF01.md` Nachtrag 2 und
`docs/api/_emutos-config.md` Abschnitt „Classification" fuer die
vollstaendige Tabelle mit allen Fundstellen. Keine Produktcodeaenderung.
