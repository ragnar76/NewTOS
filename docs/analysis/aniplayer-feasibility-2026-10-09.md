# Aniplayer port feasibility — 2026-10-09

Scope: can Aniplayer (Didier Méquignon) be ported to NewTOS? Read-only research.
Outcome: **No. Aniplayer has no published main-program source.** Points 2–5 of the
brief are therefore moot for Aniplayer itself; the weight is on point 6
(alternatives) and on what a minimal own player would need.

Everything below was read this session (URLs fetched 2026-10-08/09). Anything not
read is marked **unverified**.

## 1. Where is the source? — it is not public

* Official site: `http://didier.mequignon.free.fr/` (index, French) and
  `index-e.htm` (English). Latest version listed: **Aniplayer 2.23.1, 16 Aug 2013**,
  archive `files/aniplay.zip` (Content-Length 2,899,040 bytes, Last-Modified
  14 Sep 2018). Archived copy: web.archive.org snapshot 2022-01-02 shows the same list.
* Licence statement on that page: "Aniplayer is a Freeware" (`index-e.htm`, line
  "Aniplayer is a Freeware."). AtariUpToDate (`atariuptodate.de/en/1637/aniplayer`)
  says the same: v2.23.1, 01.08.2013, licence "Freeware". Freeware, not open source.
  No licence text for redistribution of modifications exists.
* The page marks "Source ..." only for *other* programs of the author
  (CD player CPX: "Source C", Horloge Parlante etc.: "Sources et programme en
  Français", one "Source and program in assembler"). **Aniplayer has no such line.**
* I downloaded `aniplay.zip` and listed all 147 files (6,563,675 bytes unpacked).
  Main program is the binary `ANIPLAY.PRG` (637,514 bytes) plus `SETUP.APP`.
  No source of the player. `PROG/` holds only 37 files / 10,375 lines of
  *developer material for the SLB interfaces* (headers, `LIBHEAD.S` stubs,
  `SLB_CALL.S`, small examples `STARTANI.C`, `MANIPLAY.C`, `SYNCHRO.CPX`,
  `EIFFEL.LCD/LCD.C`, `SLB/*/*.C` wrappers). `PROG/SLB/README.TXT` points to
  upstream ffmpeg / xiph / ijg / projectmayo for the library sources.
* The bundled codec libraries are binary `.SLB` shared libraries
  (m68k MiNT/MagiC shared-library format), e.g. `AVCODEC/68020/AVCODEC.SLB`
  (1,296,068 bytes) = "libavcodec ffmpeg-cvs-2003-06-19, gcc 2.95.2 -m68020"
  (`AVCODEC/*/README`), licence LGPL 2.1 (`AVCODEC/COPYING`); `JPEG.SLB` = libjpeg 6b;
  `VORBIS.SLB`, `VORB_ENC.SLB`, `LAME_ENC.SLB`, `LCD.SLB`. Their sources are the
  upstream projects, not Aniplayer's.
* GitHub: API repo search for `aniplayer atari` / `aniplayer mequignon`: 0 hits
  (the "aniplayer" repos found are unrelated: Android/Web/SFML). Unverified: whether
  the author handed sources privately or to a forum thread; I found no pointer.
  Contacting the author (`didier.mequignon@free.fr`) is the only route and would
  at best yield a licence for m68k + DSP-assembly code that was never designed for a
  clean retarget (see 2/4: 640 KB binary, DSP, Videl, MFP — unverifiable without source).

Conclusion for 1: stop. Aniplayer cannot be ported under our rule "app sources
stay unmodified, fixes in overlays" because there are no sources.

## 2–5. Aniplayer code inventory / formats / hardware / silent path

Source unavailable; therefore **not analysed**. What the *documentation* (not code)
says, labelled as vendor claims, not source facts (`index-e.htm`):

* Formats: AVI (VfW), MOV (+VR), FLI, RM, MPEG-1/2 (video with sound); audio
  AVR/WAV/AIF/DVS/RA/MP1/MP2/AC3/OGG; JPEG viewer.
* Uses DSP56001 on Falcon for picture compression and MPEG audio, Coldfire MAC on
  FireBee for MP3, VIDIX/Radeon for YUV scaling on CT60, D2D sound via YAMAHA /
  PSOUND / STARTRACK / MilanBlaster / GSXB / XBIOS cards, optional external clock.
* Runs as PRG/ACC with GEM or TTP without VDI; "needs 68020 or more for lots of codecs".
* Codec libraries loaded as SLB shared libs (binary-only in the archive).

Consequences for NewTOS that follow from the *delivered files* (inference, not
source): the player expects m68k SLBs (we cannot load them; m68k userland is a later
phase), the decoders are ffmpeg-2003 compiled for m68k, and the sound drivers are
hardware drivers. None of this is portable without the source. Nothing about display
path (vro_cpyfm vs. direct screen) or silent playback can be stated for Aniplayer.

## 6. Alternatives with verifiable source

### 6.1 `MedourMehdi/mm_mint_ffmpeg` — Atari/MiNT GEM player (verified, cloned and measured)

* https://github.com/MedourMehdi/mm_mint_ffmpeg, HEAD `b90c5ce` (2026-08-21), 1 commit
  in the shallow clone (history depth unverified). Licence **GPL-3.0** (`LICENSE`,
  GitHub API). Active in 2026.
* Size: `mm_mint_ffmpeg.cpp` 2,888 lines (single file), `makefile` 45 lines,
  `README.MD` 368 lines, `CODEC_HISTORY.md` 199 lines, icons in `bin/ico/`.
* Language: **C++** (`m68k-atari-mint-g++`, makefile:2), no assembler, no DSP code.
  Built `-m68020 -m68881 -O3 -ffast-math` (makefile `CFLAGS`).
* Dependencies (`makefile` `LDLIBS`): libgem (gemlib), pthread, libpng16, zlib,
  libm, **FFmpeg** (avformat/avcodec/avutil/swscale/swresample) and the long
  external codec chain (x264, x265, vpx, aom, theora, opus, vorbis, lame, fdk-aac,
  webp, lcms2, xml2, ssl, freetype, ...). The FFmpeg libs are *not* in the repo
  (README: "compiled for m68k-mint"). Decoders are FFmpeg's C code, so any format
  FFmpeg decodes is available if FFmpeg itself builds.
* Display path (read): GEM window; frame -> `sws_scale` into an MFDB buffer
  (`mm_wi_mfdb`, `fd_nplanes = work_out_extended[4]` from `vq_extnd`, lines 920,
  1160) -> `vro_cpyfm(handle, S_ONLY, ...)` inside the `wind_get WF_FIRSTXYWH /
  WF_NEXTXYWH` rectangle loop with `wind_update(BEG/END_UPDATE)` (lines 817-832,
  1496-1517, 2473-2503). No direct screen poking in the video path. README: needs
  >= 16-bit colour (RGB565), 24/32-bit supported. This is exactly the path NewTOS
  already serves for zView/TeraDesk-class apps.
* Hardware coupling (read, all in audio/timing):
  * Sound: `Locksnd/Sndstatus/Setmode/Soundcmd/Devconnect/Setbuffer/Buffoper`
    (lines 531-546, 1975-2059), STRAM buffer via `Mxalloc(..., MX_STRAM)`.
    Falcon external-clock detection pokes `0xFFFF8930/8932` inside `Supexec` with
    inline m68k asm (lines 447-500); a Timer-A ISR `timerA()` (line 719-723) clears
    `0xFFFFFA0F`; `enableTimerASei` pokes `0xFFFFFA17` (line 148).
  * Timing: `get200hz()` reads `0x4BA` via `Supexec` (lines 146-147); the video
    thread paces by `_hz_200` **when there is no audio stream**
    (`if (g_snd.stream_index < 0) time_200hz_start = ...`, lines 2363-2370, and
    pace loop 2419-2426), otherwise it syncs to the DMA sample position (line 2376+).
    So a **silent path already exists in the code**: audio threads and DMA init are
    only created `if (g_snd.stream_index >= 0)` (lines 2833-2837); video-only files
    take the 200 Hz pacing. A "mute audio stream of a file that has one" switch
    does not exist (README option list has no such flag; verify by forcing
    `stream_index = -1` — **unverified**, would be an overlay edit).
  * Cookie: `Getcookie(C__MCH, ...)` for audio clock tables (line 437).
  * Threads: 5 `pthread_create` (eventloop, audio, demux/decode, video display,
    audio UI), `pthread_yield_np`, mutex. Video display thread issues AES/VDI calls
    from a second thread (lines 817, 2476) — a conflict for a single-tasking AES
    (**unverified** how NewTOS would behave; needs deeper analysis).
* NewTOS fit: `docs/sdk/01-overview.md` / `05-abi-pitfalls.md` B4: `Supexec(f)` is a
  direct call of `f` with no privilege, so `get200hz` through `Supexec` works
  (`docs/sdk/07-mintlib-limits.md:187` documents `clock()` reading `_hz_200` live).
  The raw MMIO pokes (`0xFFFF89xx`, `0xFFFFFAxx`) would fault; they are confined to
  the audio path, which a silent build never reaches.
* Blockers specific to this one: (a) C++ — `docs/sdk/13-zview.md:102` lists xpdf as
  "Not built", no evidence in docs/sdk of libstdc++/C++ support (**unverified**);
  (b) FFmpeg + its transitive libs would have to be built for AArch64/NewTOS
  libc (a big port; FFmpeg is C and portable, but its configure/threads/assembly
  options must be switched off; zero precedent in `docs/sdk/11` which has only
  zlib/libpng/libjpeg/giflib/libexif); (c) pthread: mintlib `thread.c` builds
  (`07-mintlib-limits.md:53`) but a pthread library is not documented
  (**unverified**); (d) GPL-3.0 (acceptable for us only if the project licence is
  compatible — decision for the lead); (e) `-m68881` FPU code is irrelevant on AArch64.
* Sibling `MedourMehdi/mm_mint_mp4` (C++, GPL-3.0, last push 2023-10-31) — GitHub
  description only; **not read**. Same author also has `mm_recoil` (RECOIL image
  decoder GUI), `Splay_Plugins` (C, sound plugins), `SDL` / `SDL2_Sample_App`
  (Atari-targeted) — descriptions only, **not read**.

### 6.2 `phoboslab/pl_mpeg` — single-header C MPEG-1 decoder (verified, header read)

* https://github.com/phoboslab/pl_mpeg, `pl_mpeg.h` 4,439 lines, `SPDX-License-Identifier:
  MIT` (header line 3; GitHub API shows "None" because the licence is in the header,
  not a LICENSE file). Pure C: includes only `stdint.h`, `string.h`, `stdlib.h`,
  `stdio.h`. No threads, no SIMD/asm/pthread references (grep: none). MPEG-1 video
  + MP2 audio + MPEG-PS demuxer.
* API: `plm_create_with_filename`, `plm_decode`, `plm_set_audio_enabled(plm, 0)`
  (header line 349; comment at 427 says to disable audio when only decoding video),
  `plm_frame_to_rgb/rgba/bgra` (lines 747-750), `plm_set_loop`. So **video-only is a
  first-class mode**. It uses `float`/`double` (101 lines mention them) — fine on
  AArch64 hardware FP; the SDK's soft-float note (`07-mintlib-limits.md:152`) concerns
  `long double` only.
* Fit: 1 file, C, no OS coupling, no sound needed, no Falcon hardware. Output is
  YUV/RGB buffer that we blit through `vro_cpyfm` like zView does. Limits: only
  MPEG-1 (+MP2) in a program stream; no AVI/MOV/FLI/Cinepak container.

### 6.3 Others (not verified)

* FFmpeg upstream (C, LGPL/GPL): exists, not fetched here; see 6.1 for the cost.
* FLI/FLC, Cinepak, MSV1 standalone C decoders: I found no verifiable Atari-world
  C source for them in this session. Do not assume; if wanted, the FFmpeg
  `libavcodec` tree has C decoders (cinepak, msvideo1, flic) — **unverified** by
  reading, named from general knowledge only.
* Animated GIF is already decodable with giflib 6.1.3 which is in the SDK
  (`docs/sdk/11-codec-libraries.md`); that gives a zero-new-dependency animation
  path (no sound), though it is a still-image library used sequentially.

## Porting blockers (ranked) — for a NewTOS video player in general

1. **No Aniplayer source** (hard stop for Aniplayer itself).
2. **No audio stack**: no XBIOS DMA sound / codec, no DSP; a player must run
   silent or wait for a sound driver. Only the pl_mpeg/ffmpeg silent paths avoid it.
3. **Choice/cost of decoder**: FFmpeg = large port with no SDK precedent; pl_mpeg =
   tiny but one format only.
4. **C++ and pthread** support in the SDK for mm_mint_ffmpeg (**unverified**).
5. **Second-thread AES/VDI calls** in mm_mint_ffmpeg versus NewTOS single-tasking AES
   (**unverified**, needs analysis); a single-threaded pl_mpeg player avoids it.
6. **Timing**: single core; `_hz_200` is live and documented, resolution 5 ms is
   enough for 25-30 fps frame pacing (fact: 200 Hz tick; the adequacy is inference).
7. **Display throughput**: 32-bpp VDI backend with `vro_cpyfm` per frame; speed of
   that blit on the Pi is unmeasured here (unverified).
8. Licence: GPL-3.0 (mm_mint_ffmpeg), LGPL (FFmpeg), MIT (pl_mpeg).

## Effort estimate (rough, only for the option I recommend: own player on pl_mpeg)

| Part | Size | Note |
|---|---|---|
| GEM shell (window, redraw, close, file selector, `evnt_multi` with timer) | S | EmuTOS-style; zView/TeraDesk already prove the AES path |
| Container parsing | S | pl_mpeg demuxes MPEG-PS itself |
| MPEG-1 video decode | S | pl_mpeg compile + overlay for config; no asm |
| MP2 audio decode | S to compile / blocked to play | decode works, no output device |
| Other decoders (AVI/MOV/FLI/Cinepak/H.264...) | L | needs FFmpeg port or separate decoders |
| Display path (YUV->RGB -> MFDB -> `vro_cpyfm` clipped by rectangle list) | S-M | pattern from mm_mint_ffmpeg lines 817-832, 1496-1517 |
| Timing (frame pacing from `_hz_200`) | S | pattern from mm_mint_ffmpeg lines 2363-2426 |
| Audio | L (blocked) | requires the not yet existing sound driver |
| Porting mm_mint_ffmpeg as is | L | FFmpeg + C++ + pthread + GPL, see blockers 3-5 |

## Recommended minimal first milestone

"Silent playback of an MPEG-1 program stream in a GEM window": vendor `pl_mpeg.h`
(MIT) unmodified, a small own C GEM program (or an overlay-free new app under
`sdk/apps/`) that calls `plm_set_audio_enabled(plm, 0)`, converts frames with
`plm_frame_to_rgb` into an MFDB sized to the work area and blits with `vro_cpyfm`
over the `WF_FIRSTXYWH` rectangle list, paced by `_hz_200`. Acceptance: a ~352x288
test clip plays in a window at the clip's frame rate without hang; measure frames
dropped and blit cost on QEMU and the Pi (do not assume the numbers).
That milestone also settles the unknowns of blocker 7. A second step (decision for
the lead): attempt FFmpeg only if more formats than MPEG-1 are required.

## Open questions / unverified

* Whether the author would publish or license Aniplayer sources (no evidence found).
* C++ / libstdc++ and pthread availability in the NewTOS SDK.
* AES/VDI calls from a second thread under NewTOS.
* Contents of `mm_mint_mp4`, `mm_recoil`, `Splay_Plugins`.
* FFmpeg decoder availability claims in 6.3 (general knowledge, not read).
* Whether a mute-audio override works in mm_mint_ffmpeg (not run).
