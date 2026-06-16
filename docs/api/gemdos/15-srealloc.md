# `Srealloc` — GEMDOS 0x15

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/umem.c:358` (`srealloc`). Dispatched from
> `bdos/bdosmain.c:201`: `{ F(srealloc), 0, 2 }` (returns LONG, 1 LONG arg).

## EmuTOS source (essence)

```c
void *srealloc(long amount)
{
    if (!has_videl) return (void *)EINVFN;
    if (video_ram_size == 0) return NULL;

    /* compute available video-ram = current vram + adjacent free md */
    for (md = last = pmd.mp_mfl; md; last = md, md = md->m_link) ;
    if (last->m_start + last->m_length == video_ram_addr) {
        available = last->m_length + video_ram_size
                    - EXTRA_VRAM_SIZE - FREESPACE_KLUDGE;
        realloc = TRUE;
    } else {
        available = video_ram_size - EXTRA_VRAM_SIZE;
        realloc = FALSE;
    }

    if (amount < 0L) return (void *)available;     /* query */
    if (!realloc)    return NULL;                  /* not contiguous */

    amount = (amount + 255UL) & ~255UL;            /* round up */
    amount += EXTRA_VRAM_SIZE;
    if (amount > available) return NULL;

    last->m_length += video_ram_size - amount;     /* shrink free md */
    video_ram_size  = amount;
    video_ram_addr  = last->m_start + last->m_length;
    bzero(video_ram_addr, video_ram_size);
    return (void *)video_ram_addr;
}
```

`Srealloc` is an undocumented Falcon-TOS call that resizes the screen
memory by re-using the free GEMDOS block immediately below the current
video RAM. The whole function is inside `#if CONF_WITH_VIDEL` —
without VIDEL it does not exist.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x15` |
| LONG args | 1 |
| Stack frame | `+2: amount (L)`, `+0: 0x0015` |
| Dispatch | `funcs[0x15]` |

## Inputs

| Pos | Name      | Type | Range / meaning                              |
|-----|-----------|------|----------------------------------------------|
| 0   | `amount`  | LONG | `< 0` query / `≥ 0` requested screen-RAM size |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (pointer or error) |

| Case                | Return                                       |
|---------------------|----------------------------------------------|
| no VIDEL            | `EINVFN` (-32)                                |
| `video_ram_size==0` | `NULL`                                       |
| query (`amount<0`)  | available bytes (LONG, not pointer)           |
| reallocation OK     | new `video_ram_addr` pointer                  |
| not contiguous      | `NULL`                                       |
| amount > available  | `NULL`                                       |

## Errors

`EINVFN` only when `has_videl == 0`. All other failures return `NULL`,
not a negative error code — this is the Falcon ABI quirk.

## Edge cases

### 1. Only contiguous free block can grow vram

If the `mp_mfl` free-list does not end at exactly `video_ram_addr`,
reallocation is forbidden and `Srealloc(n)` returns `NULL`. Programs
must call `Srealloc(-1)` first to learn whether resizing is possible
at all.

### 2. `FREESPACE_KLUDGE = 256`

The last free MD always retains at least 256 bytes so the MD itself
need not be freed (which would be messy). This rounds `available`
down by 256.

### 3. `EXTRA_VRAM_SIZE` (Falcon)

Defined in Falcon screen.h; usually 256 bytes. Adds slack for VIDEL
overscan and is included in the actual allocation.

### 4. Allocation rounded up to 256

Even if the caller asks for 320 bytes, the function rounds to 512 +
EXTRA_VRAM_SIZE. Compatible with TOS4 behaviour.

### 5. New screen is zeroed

`bzero(video_ram_addr, video_ram_size)` — programs see a black screen
immediately after a successful resize.

### 6. Query result is a "pointer-shaped LONG"

EmuTOS returns `(void *)available`, so a successful query at e.g.
0x60000 bytes returns a valid pointer-sized value. Programs must
interpret the LONG as a number, not dereference it.

## Dependencies

- `has_videl` (boot detection).
- `video_ram_addr`, `video_ram_size` (`bios/screen.c`).
- `pmd.mp_mfl` — GEMDOS memory descriptor free-list.
- `bzero` (`util/memmove.S`).

Used by: Falcon-aware applications setting up a custom screen mode
(e.g., demos requesting 1 MB linear vram).

## Cross-references

- **Compendium:** `Srealloc` undocumented; some third-party docs call
  it "Falcon screen reallocation".
- **Profibuch:** documents `Srealloc` informally.
- `docs/api/gemdos/memory/4a-mshrink.md` — related but generic memory
  shrink.
- `docs/api/xbios/05-setscreen.md` — pairs with `Setscreen` in Falcon
  bootflows.

## Configuration

- `CONF_WITH_VIDEL`. NewTOS profile: **disabled** — function returns
  `EINVFN` unconditionally.

## NewTOS implementation notes

(Empty during Phase 00.)

NewTOS has no VIDEL and no resizable framebuffer-from-RAM model; the
VideoCore manages the framebuffer. `Srealloc` is wired to return
`EINVFN`, which is exactly the Falcon-without-VIDEL behaviour. Programs
that detect this gracefully fall back to `Setscreen` /
`Physbase`/`Logbase` and run on the VideoCore framebuffer.
