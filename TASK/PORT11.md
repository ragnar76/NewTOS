# PORT11 — SDK-Doku auf den Stand nach E9 bringen (`malloc` ueber `Malloc`, crt0)

Status: **TEILWEISE** (`docs/sdk/07` `malloc`-Zeile und libc-crt0-Absatz in `docs/sdk/03` nachgezogen, `861ad038`/`57a71b80`; offen: Plain-crt0-Absatz `docs/sdk/03` Z. 80–84 nennt weder `Mshrink` noch Accessory-Zweig, SP = `p_hitpa` ist Stand vor E9) · Prioritaet: **P3** · Typ: Doku — Nebenfund der E9-Nacharbeit ([ARCH03](ARCH03.md))

`docs/sdk/07` (Zeile `malloc`) und `docs/sdk/03` (Plain-crt0) beschreiben den
Stand vor E9. Nachziehen: TPA = groesster freier Block, `__libc_crtinit`
schrumpft per `Mshrink`, `malloc` ueber GEMDOS `Malloc`, crt0 endet mit
`exit()`.
