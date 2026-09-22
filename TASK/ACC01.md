# ACC01 — Quellenanalyse: der vollstaendige `.ACC`-Lebenszyklus in EmuTOS

Status: **OFFEN** · Prioritaet: **P2** · Typ: Quellenanalyse (Researcher) — Voraussetzung fuer ACC02–ACC05

Abhaengigkeiten: —

## Warum

`.ACC`-Unterstuetzung fehlt bei uns fast vollstaendig (Anwender 2026-09-22).
Vorhanden ist nur `menu_register` (35), das Titel und `ap_id` in einer
6er-Tabelle ablegt (`tos-api/src/aes/menu.rs`), sowie die Beschreibung von
`mn_cleanup`/`AC_CLOSE` in der Spec. Es fehlen: Laden, Prozessmodell,
Desk-Menue-Anbindung, `AC_OPEN`/`AC_CLOSE`-Protokoll, Lebenszyklus ueber
Programmstarts hinweg.

## Auftrag

Den Lebenszyklus aus der gepinnten Quelle extrahieren, je Schritt mit
`datei:zeile`, nach `docs/SOURCE-ANALYSIS-PROTOCOL.md`:

1. **Zaehlen und Laden**: `count_accs` (`aes/geminit.c:265-284`, `\*.ACC`
   ueber `dos_sfirst`/`dos_snext`, `FA_RO`, `NUM_ACCS = 6`
   `include/sysconf.h:37`), `load_one_acc` (`:236-257`: `dos_open`, `pgmld`,
   `pstart(gotopgm, name, addr)`), `load_accs` (`:307-315`), `free_accs`
   (`:288-303`), Startreihenfolge in `run_accs_and_desktop` (`:654-745`:
   `load_accs` **vor** `sh_tographic`, `dsptch()`, dann
   `wait_for_accs(AP_MESAG)` `:737`, `sh_main`, danach `free_accs`).
2. **Bootflag**: `BOOTFLAG_SKIP_AUTO_ACC` (`:268-270`) — woher es kommt
   (`bios/`), welche Taste, was es sonst noch ueberspringt.
3. **Prozessmodell**: `pstart`/`AESPD`, wie ein Accessory zwischen
   `appl_init` und seinem ersten `evnt_multi` steht; was `wait_for_accs`
   genau abwartet (`:588-630`, `AP_MESAG`/`AP_ACCLOSE`, `WAIT_TIMEOUT 500`).
4. **Menueanbindung**: wie die per `menu_register` gemeldeten Titel in
   `THEDESK` erscheinen (`aes/gemmnlib.c` `mn_bar`/`mn_register`
   `gemgrlib`?), wer `AC_OPEN` postet (`gemctrl.c`/`gemmnlib.c` — Fundstelle
   belegen), mit welchen `msg`-Woertern (`ap_gaccs`-Index?), und wie
   `AC_CLOSE` beim Programmstart/Ende laeuft (`mn_cleanup`
   `gemmnlib.c:740-760`, Aufrufer `ap_exit` `gemaplib.c:254`, `sh_ldapp`
   `gemshlib.c:652`).
5. **Was ein Accessory selbst tun muss** (fuer unsere spaeteren Tests):
   `appl_init`, `menu_register`, `evnt_multi`-Schleife, Reaktion auf
   `AC_OPEN`/`AC_CLOSE`, **kein** `appl_exit`/`Pterm`. Beispiel aus
   EmuTOS/Fremdquelle benennen (z. B. ein minimales `.ACC` aus dem
   FreeMiNT-/Atari-Umfeld, nur als Referenz zitiert).

## Abnahme

`docs/api/aes/accessories.md` (neu): Lebenszyklus, Zustandsdiagramm in Worten,
Vertrag je AES-Funktion, alle Zitate mit `datei:zeile`; Liste der NewTOS-Luecken
mit Verweis auf ACC02–ACC05. Keine Codeaenderung.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
