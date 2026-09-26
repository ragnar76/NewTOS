# measurement-2026-09-26-desk43-u2

Belege zu **DESK43 / U2** — „Der Zustandskanal: `app_start`/`app_save` über
`shel_get`/`shel_put`" (`docs/analysis/desktop-userspace-plan-v2.md` §5 U2).

Baum: `desk-rebuild` @ `814de2c` + die U2-Änderung (unversioniert im Baum).
EmuTOS-Referenz: `references/emutos`.
Spec: `docs/api/aes/shell-loop.md` §10, `docs/api/aes/scrap-shell.md` §2,
`docs/api/desktop/deskapp.md` §„`app_start()` — full AES/GEMDOS call sequence".
Alles headless in QEMU `raspi4b` (640×480, Dev-Bau). **Keine echte Pi 4, nichts
geflasht, `/Volumes/BOOT` nicht angefasst.**

```
serial/       die Seriellprotokolle aller acht Läufe
screendumps/  die QMP-Screendumps dazu
inf/          die drei NEWDESK.INF-Fassungen (vom Kernel geschrieben bzw. daraus abgeleitet)
method/       der Messaufbau (Scratch-Stand, von Hand zurückgedreht)
build/        Bauen, clippy, Hosttests, Driftprobe
```

## Die Zahlen

| | |
|---|---|
| **Driftprobe** | **47** (A 21 · B 21 · C 5) — **unverändert** gegenüber U1 |
| Bauen, beide Boards, je frisch | tos-api **64** / kernel **94** Warnungen (Basis 64/94, Delta 0) |
| clippy, beide Feature-Sätze | tos-api **80** / kernel **125** (Basis 80/125, Delta 0); in `kernel/src/desk/` **null** Treffer |
| `cargo run -p xtask -- test` | **956 / 0 / 1** (Basis 956/0/1) |

Die Driftprobe bleibt stehen, weil U2 nichts entfernt: sie zählt `tos_api::`-
und `crate::`-Pfade in `kernel/src/desk/`, und die beiden neuen Bindungen
(`shel_get`/`shel_put`) gehen über `Aes::call` → `svc #2` und nennen keines von
beidem. Der Zustandskanal ist der *Ersatz*, der U3 das Entfernen erlaubt
(Plan §6.3: erst der Ersatz, dann die Entfernung).

## Die Läufe

| Lauf | Kernel | `C:\NEWDESK.INF` | Shell-Puffer | Ergebnis |
|---|---|---|---|---|
| `run1-probe-panic` | U2 + Sonde | Platzhalter | leer | Sonde stürzte in ihrer eigenen NUL-Suche ab — die Nutzdaten davor sind gültig und aufgehoben |
| `run2-first-app_save` | U2 + Sonde | Platzhalter | leer | `app_save(FALSE)` legt 755 Byte ab (128 Präfix + 627 Text + NUL); `app_save(TRUE)` schreibt die Datei → `inf/newdesk-from-app_save.inf` |
| `head-placeholder` | **814de2c** | Platzhalter | leer | Referenzbild |
| `head-file` | **814de2c** | `#W … C:\ACC\@` | leer | Referenzbild, ein Fenster |
| `run3-file` / `final-file` | U2, **ohne** Sonde | `#W … C:\ACC\@` | leer | Fenster `C:\ACC\` — die Datei, zweite Stufe der Kaskade |
| `run4-buffer` | U2 + Sonde | `#W … C:\ACC\@` | `#a000000\r\n` + INF mit `#W … C:\AUTO\@` | Fenster **`C:\AUTO\`** — der Puffer schlägt die Datei |
| `run5-noinf` | U2, ohne Sonde | **gelöscht** | leer | Standardumgebung, pixelgleich zu `run6` |
| `run6-placeholder` / `final-placeholder` | U2, ohne Sonde | Platzhalter | leer | Standardumgebung |
| `run7-launch` | U2 + Sonde | `#W … C:\ACC\@` | erst geimpft, dann **genullt** | `run_gem_app("C:\ARGVTEST.NAP")` → das Programm läuft und endet → der Puffer ist wieder gefüllt |

## Was damit bewiesen ist

**(1) Die Kaskade `Puffer → Datei → Vorgabe` und der 128-Byte-Versatz.**
`run3-file` und `run4-buffer` fahren **dasselbe Plattenabbild** mit derselben
`NEWDESK.INF` (`#W … C:\ACC\@`). Der einzige Unterschied ist der geimpfte
Shell-Puffer. `run3` öffnet `C:\ACC\`, `run4` öffnet `C:\AUTO\` — also liest
`app_start` **zuerst** den Puffer (`deskapp.c:832-836`).

Der Versatz ist dabei mitbewiesen: das Präfix im Puffer ist `CP_SHELL_INIT` =
`#a000000\r\n` (`aes/geminit.c:97`) und beginnt mit `#`. Läse `load_and_apply`
bei Offset 0 statt 128, bestünde die Signaturprüfung auf den Steuerfelddaten
und der Desktop käme mit Unsinn statt mit `C:\AUTO\` hoch.

**(2) Das Präfix überlebt jedes Speichern.** `run4`:
`[u2] prefix preserved = true`, und der Nahtdump
`buf[120..136] = \0…\0 #R 02\r\n#` zeigt, dass der INF-Text exakt bei 128
anfängt (`deskapp.c:1123-1124`).

**(3) Der echte Startpfad schreibt den Puffer.** `run7-launch` nullt den Puffer
(`buffer wiped: all zero = true`), ruft dann das **unveränderte**
`main::run_gem_app` — `desktop_relinquish` → `launch` → `desktop_resume` —,
`ARGVTEST.NAP` läuft (`argc 1`) und endet, und danach steht der INF-Text mit
`#W 00 00 02 04 26 0C 00 C:\AUTO\*.*@` wieder im Puffer. Das ist
`deskmain.c:2155` an seiner Stelle. `screendumps/run7-after-launch.png` zeigt
den vollständig wiederhergestellten Desktop.

**(4) Kaltstart ohne `NEWDESK.INF`.** `run5-noinf` ist **byteweise identisch**
zu `run6-placeholder` (0 abweichende Bytes nach PNG-Dekompression) —
unveränderte Standardumgebung (`deskapp.c:839-840`).

**(5) *Save desktop* schreibt weiter korrekt, `fsck_msdos -n` sauber.**
`app_save(TRUE)` schrieb in `run2` 627 Byte und in `run4` 635 Byte;
`inf/newdesk-after-run4.inf` enthält die `C:\AUTO\`-Fensterzeile und **kein**
Präfix (`save_to_disk(inf_data, len-CPDATA_LEN)`, `:1297`).
`fsck_msdos -n` auf dem Abbild nach `run4`: Phase 1–3 ohne Befund, rc 0.

**(6) Keine Regression im Bootbild.** `final-placeholder` gegen
`head-placeholder` und `final-file` gegen `head-file`: je **0 abweichende
Bytes**. Der Referenzkernel wurde in einem abgetrennten `git worktree` auf
`814de2c` gebaut (der geteilte Baum blieb unberührt) und danach entfernt.

## Die Vorhersage des Plans

> *„Der erste Bruch ist das `CPDATA`-Präfix … Zweiter Kandidat: `cnx_put()`
> schreibt die `#W`-Zeilen heute in `G.g_cnxsave`; sie müssen in den
> serialisierten Text."*

**Beide sind nicht gebrochen** — sie waren die beiden Stellen, an denen die
Umsetzung von vornherein gegen die Quelle gelegt wurde, und die Messung
bestätigt das, statt einen Fehler zu finden. Das Präfix wird bei 128
respektiert und unverändert durchgereicht; die `#W`-Zeilen stehen im Text, weil
`build_from_state` ohnehin aus `g_cnxsave` liest (`deskapp.c:1193-1203`) — das
war schon vor U2 so und musste nur an den Puffer angeschlossen werden.

## Was **nicht** gilt: Beweispunkt 1 des Plans

Der Plan verlangt: *„A106-Restart auslösen — der neue Desktop kommt mit
denselben Fenstern **aus dem Shell-Puffer**, nicht aus der Datei."*

**Das ist EmuTOS-widrig und darf nicht eintreten.** `aes_restart` nullt vor
jedem neuen `gem_main` den kompletten `THEGLO`-Bereich
(`aes/gemstart.S:86,99-106`, `jsr _size_theglo` → `_bzero_nobuiltin` auf `_D`;
`size_theglo` = `sizeof(THEGLO)`, `aes/geminit.c:149-152`), und `g_shelbuf`
**ist** ein `THEGLO`-Mitglied (`aes/gemlib.h:129`). Nach einem
Auflösungswechsel hat EmuTOS' Desktop also einen leeren Puffer und liest die
Datei — genau die zweite Stufe der Kaskade.

Unser Baum macht das bereits richtig: `aes::reinit()` ruft `shell::reinit()`
(`tos-api/src/aes/mod.rs:186`), das `(*S.buf.get()).fill(0)`
(`tos-api/src/aes/shell.rs:801`) ausführt — mit genau dieser Begründung im
Doc-Kommentar. Der A106-Pfad (`kernel/src/lib.rs:474-475`) ist damit die
**Datei**-Stufe, nicht die Puffer-Stufe.

Der Kanal trägt in EmuTOS über den Tod des *Desktop-Prozesses innerhalb einer
AES-Sitzung* — den Start einer Anwendung (`deskmain.c:2155` → `app_start`
beim Wiedereintritt) —, nicht über den Neustart der Sitzung selbst. Lauf 7
fährt genau diesen Pfad; er ist der richtige Ersatz für Beweispunkt 1.

## Messaufbau

`method/scratch-desk-mod.diff` + `method/scratch-u2probe.rs` sind der
Scratch-Stand der Läufe 1/2/4/7 (zwei Haken in `desk::run` und ein
Sondenmodul). Zusätzlich war in `kernel/src/desk/main.rs` `run_gem_app` für
Lauf 7 auf `pub(super)` gesetzt. **Alles drei ist von Hand zurückgedreht** —
kein `git checkout`/`restore`/`stash` (`git status` zeigt nur die vier
Produktdateien). Die Läufe `run3`, `run5`, `run6`, `final-*` liefen auf dem
**ausgelieferten** Kernel ohne jede Sonde.

Gefahren mit `measurement-2026-09-22-a106/method/run.sh` (QEMU `raspi4b`,
`-display none`, QMP-Screendump am Serienmarker). Das SD-Abbild stammt aus
`tools/make-sd-image.sh` (einzige zulässige Quelle) und ist der Auflage wegen
**nicht** beigelegt.
