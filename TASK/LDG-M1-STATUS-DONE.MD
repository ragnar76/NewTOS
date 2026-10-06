# LDG-M1 — Quittung: `ex_lib` als natives `.NLD` über den echten LDG-Weg, `sinus()` gerufen

Datum 2026-10-06 · Basis `desk-rebuild`, HEAD `6141514`, Baum vorher sauber
· nicht committet · Belege [measurements/measurement-2026-10-06-ldg-m1/](../measurements/measurement-2026-10-06-ldg-m1/RUNINFO.TXT)
· Entwurf [docs/analysis/ldg-after-arch03-2026-10-06.md](../docs/analysis/ldg-after-arch03-2026-10-06.md) §7 (S2–S7)

Lead-Entscheide 2026-10-06: Q2 Quelle r133 als Tarball pinnen, ldg-20171014
offen lassen · Q3 ADR-018-Nachtrag `.NLD`, Desktop bleibt `Doc` · Q4 eigener
headless Client `LDGTEST.NCP` · Q5 `ldg_callback`/`stdcallgcc.o` weglassen,
„nicht vorhanden“ · Q6 `_stksize` wie mintlib · Q8/Q9 kein TSR,
`PF_STANDARD` (PROT_P, Besitzer = Client). S1 (environ aus `p_env`) war
PORT09.

## Umsetzung

* **Quelle gepinnt:** `sdk/ldg/ldg-svn-r133.tar.xz` = `Applications/ldg-src/ldg`
  unverändert (212 Dateien), sha256 in `sdk/ldg/ldg-sources.sha256`, Datei-
  Manifest `ldg-svn-r133.files.sha256` (gegen frisches Auspacken 212/212 OK).
* **Overlays** (`sdk/ldg/overlay/src/devel/`, je mit Kopfkommentar und
  file:line): `ldgload.c` — `p_env`-Cast an `:76` (die übrigen `p_env`-Stellen
  gehen nur an `Mfree`, dessen Binding nach `long` wandelt); `ldgutil.c` —
  `ldg_cookie` über `Getcookie()` statt `*(long *)0x5a0`, `ldg_cpush` leer
  (Kernel synchronisiert in `Pexec(3)`, `loader.rs:599`; `UCI` bleibt aus,
  `boot.rs:189-198`). `ldgapp.c`, `ldglib.c`, `ex_lib.c`, `exlibbnd.c`
  unverändert.
* **Bau:** `sdk/ldg/build-ldg.sh` (libldg.a, `common.mak`-OBJS ohne
  `stdcallgcc.o` → kein `ldg_callback`-Symbol, Linkfehler statt Stub),
  `sdk/ldg/build-ldg-demo.sh` (`EX_LIB.NLD`, `LDGTEST.NCP`), Bibliothek
  `ldg` in `sdk/lib-stale.sh` (Kette libc → ldg, Tarball beobachtet),
  `sdk/.gitignore`.
* **Client** `sdk/examples/ldgtest.c`: 69 Prüfungen (Kopfkommentar).
* **Abbild:** `tools/make-sd-image.sh` legt `C:\LDG\EX_LIB.NLD` und
  `C:\LDGTEST.NCP` ab, `SD_LDGTEST_AUTO=1` zusätzlich `\AUTO\` (nur
  Syntaxprüfung; für die Läufe eigene Karten über `method/seed-card.sh`).
* **Doku:** `docs/sdk/10-ldg-modules.md` (Bau eines `.NLD`, Grenzen),
  `docs/sdk/README.md`, ADR-018-Nachtrag in `docs/MEMORY.md`.
  `docs/api/gemdos/process/4b-pexec.md` unverändert (kein neues Verhalten).
* **Kernel: keine Änderung nötig gewesen.**

## Abnahme

QEMU raspi4b, `\AUTO\LDGTEST.NCP`: **`ldgtest: ALL PASS (69 checks)`** unter
`memprot=off`, `on` **und** `strict`; zusätzlich per `#Z` vom Desktop (on):
ALL PASS. `sinus(0.5)` = 0.479426 = `sin(0.5)` des Clients (Toleranz 1e-12),
`ldg_close` = 0, größter freier Block nach jedem Schließen und am Ende gleich
dem Startwert; relativer Name nach `Dsetpath(\LDG)` geht; Neuladen nach
Schließen landet an **derselben** Basepage (0x19d3000); zweites gleichzeitiges
Öffnen gibt einen anderen Deskriptor, beide funktionieren. Gegenprobe ohne
Moduldatei: sauberes FAIL, kein Speicherverlust, Boot läuft weiter.

Statisch (Basis = PORT09): Hosttests 1165/0/1, Release-Warnungen 64/94 beide
Boards, Clippy 76/125 beide, Drift 31, SDK-Hosttests 14/0/0 — unverändert.
Regression (on): S2ALL PASS, APITEST 106/0/5 (wie PORT09), QED und TeraDesk
starten; 5 Screendumps byteidentisch zu PORT09.

## Offen

* **Nur die echte Pi:** I-Cache-Kohärenz beim Laden und vor allem beim
  **Neuladen an derselben Adresse** (QEMU hat keine Caches); das
  Alias-Argument (PIPT) und `asid=on`. `LDGTEST` ist der Prüfling dafür.
* Abgleich r133 ↔ th-otto `ldg-20171014` (`ldg-cross.patch`,
  `ldg-pwait.patch`) nicht gemacht.
* `tools/make-sd-image.sh`-Block nicht ausgeführt (fester Mountpunkt).
* Nach M1: Fremdbibliotheken (M2), windom + unveränderter `demo/client` (M3),
  zView mit `plugins.c`-Overlay für `.NLD` + libshare (M4).
* `docs/STATUS.md` nicht nachgezogen (Sache des Leads beim Commit).
