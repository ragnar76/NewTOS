#include "global.h"
#include "qed.h"
#include "memory.h"
/* Heiko */
#include "hl.h"

#include <stdlib.h>	/* malloc / free -- LP64-correct libc heap (in-TPA arena) */

/*
 * ==========================================================================
 *  LP64 overlay of QED private line-buffer allocator (src/memory.c).
 *
 *  Upstream memory.c is a fine-grained sub-allocator: it grabs big arenas
 *  from GEMDOS Malloc() and hands small line blocks out of a segregated free
 *  list.  That pool logic hardcodes sizeof(pointer)==4 in three places:
 *    - the tblock union (commented "=> 12 Bytes"; two 8-byte pointers on LP64
 *      make it 24),
 *    - new_block, whose 4-byte block-list header is written with *(void**)ptr
 *      (8 bytes on LP64, overrunning the first block),
 *    - free_list, indexed by the byte-size treated as a pointer-slot offset
 *      via (char*)free_list + size, valid only when sizeof(BLOCK*)==4.
 *  On LP64 (native 64-bit GEM apps) every one of those overruns; QED faulted
 *  at startup with FAR=0xffffffff00000008 (a 4-byte pointer read at 8-byte
 *  width) inside new_block.
 *
 *  Approach (b), per the task and confirmed by the lead: the pool internal
 *  block-recycling is UNOBSERVABLE to QED editing behaviour (COMPATIBILITY:
 *  do not port unobservable ROM/RAM-scarcity tricks; QED memory.c is itself
 *  only a suballocator over the already-LP64-correct coarse allocator).
 *  What IS observable is the public contract in memory.h: MALLOC, FREE,
 *  REALLOC and the 2-byte size record REALLOC reads via MEM_SUB(adr,2).  So we
 *  keep every col_ , textring, REALLOC and get_line function 100 percent
 *  verbatim and re-back only MALLOC and FREE with the LP64-correct libc malloc
 *  and free (mintlib, drawing from the in-TPA arena crt0 sets up via
 *  __libc_init_heap; the LP64 malloc fix landed in task #100).
 *
 *  Contract preserved for the verbatim REALLOC and FREE:
 *    - MALLOC(size) returns P with the 2 bytes at [P-2] holding the block
 *      rounded size MEM_SIZE(size) (== used.size).  REALLOC compares that to
 *      MEM_SIZE(sizeof(LINE)+new_len+1) to choose grow-or-shrink in place vs.
 *      reallocate, exactly as before, so REALLOC is unchanged.
 *    - REALLOC never touches free-list bookkeeping; it only reads used.size
 *      and calls MALLOC + memcpy + FREE.
 *    - Every live block is threaded onto alloc_list so kill_memory can
 *      bulk-free the whole pool, mirroring the upstream arena walk.
 *
 *  Two LP64 corrections fall out for free:
 *    - No BLOCK_SIZE cap.  Upstream rejected size > BLOCK_SIZE(1048); with
 *      LP64 sizeof(LINE) growing 20 to 32, MEM_SIZE(sizeof(LINE)+MAX_LINE_LEN
 *      +1) = 1060 > 1048 would wrongly reject the longest legal line.  libc
 *      has no block limit; we keep only a defensive guard at the real limit.
 *    - Alignment.  The returned pointer still sits 2 bytes into the block (as
 *      upstream), so LINE 8-byte members are 2-byte-aligned.  That is safe
 *      here: SCTLR_EL1.A == 0 (alignment check off) in the kernel, and the
 *      layout exactly matches upstream, no new alignment assumption.
 * ==========================================================================
 */

#undef MAGIC
#define MAGIC		0xFED1

#define MEM_SIZE(x)	(((x)+2+3)&(~3))	/* +2 for the size header, round up to 4 */
#define MEM_ADD(x,d)	(BLOCK*)((char*)(x)+(d))
#define MEM_SUB(x,d)	(BLOCK*)((char*)(x)-(d))

/*
 * Retained only for its 'used.size' member (offset 0, 2 bytes): the size
 * record MALLOC writes and REALLOC/FREE read via MEM_SUB(adr,2).  The 'free'
 * arm is no longer used (there is no free list) but is kept so MEM_SUB casts
 * and the #if 0 debug helpers below still name a complete type.
 */
typedef union tblock
{
	struct
	{
		unsigned short magic;
		unsigned short size;
		union tblock *next;
		union tblock *prev;
	} free;
	struct
	{
		unsigned short size;		/* == MEM_SIZE(request); read by REALLOC/FREE */
	} used;
} BLOCK;

/*
 * Each live allocation is prefixed with this node and threaded onto
 * alloc_list, so kill_memory() reclaims the whole pool at once (as the
 * upstream arena walk did).  Physical layout of one allocation, from the
 * libc malloc base 'b':
 *
 *     b                      : MBLK { next, prev }   (2 * sizeof(void*))
 *     b + sizeof(MBLK)       : unsigned short size   (== returned P minus 2)
 *     b + sizeof(MBLK) + 2 = P: LINE + text          (returned to the caller)
 *
 * so MEM_SUB(P,2) is the size record and P - HDR_BYTES recovers the malloc
 * base for free().
 */
typedef struct mblk
{
	struct mblk *next;
	struct mblk *prev;
} MBLK;

#define HDR_BYTES	(sizeof(MBLK) + 2)	/* MBLK node + 2-byte size record */

static MBLK *	alloc_list;			/* all live blocks (for kill_memory) */
static bool	mem_low;			/* a malloc has failed since the last free */


static void *
MALLOC(unsigned short size)
{
	unsigned short	bsize;
	MBLK		*hdr;
	BLOCK		*adr;

	if (size > (unsigned short)(sizeof(LINE) + MAX_LINE_LEN + 1))
	{
		debug("\aMalloc: size= %d (> %d)\n", size,
		      (int)(sizeof(LINE) + MAX_LINE_LEN + 1));
		note(1, 0, FATALMEM);
		return NULL;
	}
	bsize = MEM_SIZE(size);				/* incl. 2-byte header, rounded to 4 */

	hdr = (MBLK *) malloc(sizeof(MBLK) + (size_t) bsize);
	if (hdr == NULL)
	{
		mem_low = TRUE;
		note(1, 0, FATALMEM);
		return NULL;
	}

	hdr->prev = NULL;				/* thread onto the live list */
	hdr->next = alloc_list;
	if (alloc_list != NULL)
		alloc_list->prev = hdr;
	alloc_list = hdr;

	adr = (BLOCK *)((char *)hdr + sizeof(MBLK));	/* the 2-byte size record */
	adr->used.size = bsize;
	return MEM_ADD(adr, 2);				/* == (char*)hdr + HDR_BYTES */
}

static void
FREE(void *adr)
{
	MBLK	*hdr;

	if (adr == NULL)
		return;

	hdr = (MBLK *)((char *)adr - HDR_BYTES);	/* recover the malloc base */

	if (hdr->prev != NULL)				/* unhook from the live list */
		hdr->prev->next = hdr->next;
	else
		alloc_list = hdr->next;
	if (hdr->next != NULL)
		hdr->next->prev = hdr->prev;

	mem_low = FALSE;				/* memory just came back */
	free(hdr);
}

/* =========================================================== */

void INSERT(LINEP *a, short pos, short delta, char *str)
{
	char	*ptr;
	
	ptr = REALLOC(a,pos,delta);
	memcpy(ptr, str, delta);
}

/* =========================================================== */

char *
REALLOC(LINEP *a, short pos, short delta)
{
	LINEP	col;
	short		new_len;
	BLOCK 	*adr;

	col = *a;
	if (delta == 0) 
		return (TEXT(col) + pos);

	new_len = col->len + delta;
	if (new_len < 0 || new_len > MAX_LINE_LEN)
	{
		debug("\aREALLOC: new_len= %d (> %d)\n", new_len, MAX_LINE_LEN);
		note(1, 0, FATALMEM);
		return NULL;
	}
	adr = MEM_SUB(col, 2);
	if (MEM_SIZE((unsigned short)sizeof(LINE)+new_len+1) != adr->used.size)
	{
		LINEP	new;

		new = MALLOC( (short) sizeof(LINE) + new_len + 1);
		if (new == NULL)
			return NULL;
		memcpy(new, col, sizeof(LINE)+pos);
		if (delta<0)
		{
			new->len = new_len;
			memcpy(TEXT(new) + pos, TEXT(col) + pos - delta, new_len - pos + 1);
		}
		else
		{
			memcpy(TEXT(new) + pos + delta, TEXT(col) + pos, col->len - pos + 1);
			new->len = new_len;
		}
		new->is_longest = col->is_longest;
		FREE(col);
		new->prev->next = new;
		new->next->prev = new;
		*a = new;
		new->exp_len = -1;
		return(TEXT(new)+pos);
	}
	else	/* Der Platzt in der Zeile reicht */
	{
		if (delta < 0)
		{
			col->len = new_len;
			memcpy(TEXT(col) + pos, TEXT(col) + pos - delta, new_len - pos + 1);
		}
		else
		{
			memmove(TEXT(col) + pos + delta, TEXT(col) + pos, col->len - pos + 1);
			col->len = new_len;
		}
		col->exp_len = -1;
		return(TEXT(col)+pos);
	}
}

LINEP new_col(char *str, short l)
{
	LINEP 	a;
	
	a = (LINEP)MALLOC( (short)sizeof(LINE)+l+1);
	if (a != NULL)
	{
		a->info = 0;
		a->len = l;
		a->exp_len = -1;
		a->is_longest = FALSE;
		a->hl_handle = NULL;
		memcpy(TEXT(a), str, l);
		TEXT(a)[l] = EOS;
	}
	return(a);
}

void free_col(RINGP rp, LINEP col)
{
	(void) rp;
	FREE(col);
}

/* Zeile nach WO einfgen	  */
LINEP col_insert(RINGP rp, LINEP wo, LINEP was)
{
	LINEP help;

	help = wo->next;
	help->prev = was;
	was->next = help;
	was->prev = wo;
	wo->next = was;
	if (rp)
		hl_insert( rp, was );
	return was;
}

/* Zeile am Ende anh„ngen */
void col_append(RINGP rp, LINEP was)
{
	LINEP help;

	help = LAST(rp);
	was->prev = help;
	was->next = help->next;
	help->next = was;
	rp->tail.prev = was;
	rp->lines++;
	hl_insert( rp, was );
}

void col_delete(RINGP rp, LINEP was)
{
	hl_remove( rp, was );
	was->prev->next = was->next;
	was->next->prev = was->prev;
	FREE(was);
	rp->lines--;
}

void col_concate(RINGP rp, LINEP *wo)
{
	LINEP	help, col;
	bool	absatz;

	col = *wo;
	help = col->next;
	if (col->len)
	{
		absatz = help->info&ABSATZ;
		INSERT(&col, col->len, help->len, TEXT(help));
		help->next->prev = col;
		col->next = help->next;
		if (rp) {
			hl_remove( rp, help );
			hl_update_zeile( rp, col );
		}
		FREE(help);
		if (absatz)
			col->info |= ABSATZ;
		else
			col->info &= ~ABSATZ;
		*wo = col;
	}
	else
	{
		col->prev->next = help;
		help->prev = col->prev;
		if (rp)
			hl_remove( rp, col );
		FREE(col);
		*wo = help;
	}
}

void col_split(RINGP rp, LINEP *col,short pos)
{
	LINEP	new,help;
	short	anz;
	bool	absatz, overlen;

	help = *col;
	absatz = IS_ABSATZ(help);
	help->info &= ~ABSATZ;
	overlen = IS_OVERLEN(help);
	help->info &= ~OVERLEN;
	if (pos==0)
	{
		new = new_col("",0);
		col_insert(rp, help->prev, new);
		*col = new;
	}
	else if (pos<help->len)
	{
		if (rp)
			hl_remove( rp, help );
		anz = help->len-pos;
		new = new_col(TEXT(help)+pos, anz);
		col_insert (NULL, help, new);
		REALLOC(&help,pos,-anz);
		if (rp) {
			hl_insert( rp, help );
			hl_insert( rp, new );
		}
		*col = help;
	}
	else
	{
		new = new_col("",0);
		col_insert(rp, help,new);
	}
	if (absatz) 
		(*col)->next->info |= ABSATZ;
	else 
		(*col)->next->info &= ~ABSATZ;
	if (overlen) 
		(*col)->next->info |= OVERLEN;
}

/* 
 * Wieviel WhiteSpace-Zeichen stehen am Anfang der Zeile?
*/
short col_offset(RINGP rp, LINEP col)
{
	short	pos;
	char c, *str;

	(void) rp;
	pos = 0;
	str = TEXT(col);
	c = *str++;
	while ((c=='\t' || c==' ') && pos<col->len)
	{
		pos++;
		c = *str++;
	}
	return pos;
}

short col_einrucken(RINGP rp, LINEP *col)
{
	LINEP	vor_col;
	short	length;

	vor_col = (*col)->prev;
	if (!IS_HEAD(vor_col))
	{
		length = col_offset(rp, vor_col);
		if ((*col)->len + length > MAX_LINE_LEN)
		{
			inote(1, 0, TOOLONG, MAX_LINE_LEN);
			return 0;
		}
		INSERT(col, 0, length, TEXT(vor_col));
	}
	else
		length = 0;
	return(length);
}

LINEP get_line(RINGP r, long y)
{
	LINEP	line;

	if (y < 0 || y >= r->lines) return NULL;
	if (y < (r->lines >> 1))
	{
		line = FIRST(r);
		while ((--y) >=0) NEXT(line);
	}
	else
	{
		line = LAST(r);
		y = r->lines-y;
		while ((--y) >0) PREV(line);
	}
	return line;
}

/*=========================================================================*/

/*
 * Ein Text ist als doppelt verkettete Liste implementiert
 * Am Anfang und Ende befindet sich je ein Zeilenkopf der
 * auf NULL zeigt
*/

void init_textring(RINGP r)
{
	LINEP	a;

	r->head.info = HEAD;
	r->tail.info = TAIL;
	a = new_col("",0);
	a->next = &r->tail;
	a->prev = &r->head;
	LAST(r) = a;
	r->tail.next = NULL;
	FIRST(r) = a;
	r->head.prev = NULL;
	r->lines = 1;
	r->ending = lns_tos;
	r->max_line_len = MAX_LINE_LEN;
	r->hl_anchor = NULL;
}

long textring_bytes(RINGP r)
{
	LINEP	line, ende;
	long		bytes, overlen = 0;

	line = FIRST(r);
	ende = LAST(r);
	bytes = line->len;
	while(line!=ende)
	{
		NEXT(line);
		bytes += line->len;
		if (IS_OVERLEN(line))
			overlen++;
	}

	if (r->ending != lns_binmode)
	{
		/* plus Zeilenenden: 1 Zeichen fr alle */
		bytes += r->lines - 1 - overlen;
		if (r->ending == lns_tos)
			/* fr TOS noch ein Zeichen */
			bytes += r->lines - 1 - overlen;
	}
	return bytes;
}

void free_textring(RINGP r)
{
	LINEP line, frei, ende;

	frei = LAST(r);				/* letzte Zeile */
	line = frei->prev;			/* vorletzte Zeile */
	ende = FIRST(r);				/* erste Zeile */
	while (frei!=ende)
	{
		FREE(frei);
		frei = line;
		PREV(line);
	}
	LAST(r) = frei;
	frei->next = &r->tail;
	REALLOC(&frei,0,-(frei->len));
	frei->info = 0;
	r->lines = 1;
}

void kill_textring(RINGP r)
{
	free_textring(r);
	FREE(FIRST(r));
	FIRST(r) = NULL;
	LAST(r) = NULL;
	r->lines = 0;
}

bool doppeln(RINGP old, RINGP new)
{
	LINEP	line, neu, a;
	long	lines, anz;
	bool	erg;

	erg = TRUE;
	free_textring(new);
	a	= FIRST(new);
	line	= FIRST(old);
	anz	= old->lines;

	INSERT(&a, 0, line->len, TEXT(line));
	a->info = line->info;
	NEXT(line);
	NEXT(a);
	lines = 1L;
	while (lines<anz)
	{
		neu = new_col(TEXT(line),line->len);
		neu->info = line->info;			/* ABSATZ mit kopieren */
		col_insert(NULL,a->prev, neu);
		NEXT(line);
		lines++;
		if (!ist_mem_frei())
		{
			erg = FALSE;
			break;
		}
	}
	new->lines = lines;
	new->ending = old->ending;
	new->max_line_len = old->max_line_len;
	return erg;
}

bool ist_leer(RINGP r)
{
	return(r->lines==1 && FIRST(r)->len==0);
}

/*=========================================================================*/

void kill_memory(void)
{
	MBLK	*hdr, *nxt;

	/* Bulk-free the whole pool, mirroring the upstream arena walk. */
	hdr = alloc_list;
	while (hdr != NULL)
	{
		nxt = hdr->next;
		free(hdr);
		hdr = nxt;
	}
	alloc_list = NULL;
	mem_low = FALSE;
}

bool ist_mem_frei(void)
{
	/*
	 * Upstream kept two predictive low-water flags (BS/TQ arenas) and
	 * reported exhaustion only when both fired.  With a single libc heap the
	 * honest signal is "the last malloc failed and nothing has been freed
	 * since"; the next FREE clears it.  Same observable result: report
	 * NOMEMORY exactly when the allocator is actually out of memory.
	 */
	if (mem_low)
	{
		note(1, 0, NOMEMORY);
		return FALSE;
	}
	return TRUE;
}

void init_memory(void)
{
	alloc_list = NULL;
	mem_low = FALSE;
}


/* Debug-Funktionen **********************************************************/

#if 0
void dump_freelist(void)
{
	if (debug_level)
	{
		short	i;
		
		debug("dump_freelist...\n");
		for (i = 0; i < MAX_ANZ + 1; i++)
		{
			if ((free_list[i] != NULL) && (free_list[i]->free.magic == MAGIC))
			{
				BLOCK	*p;
				short	d;
				
				p = free_list[i];
				d = 0;
				while (p)
				{
					d++;
					p = p->free.next;
				}
				debug(" [%d].size: %d, Anzahl: %d\n", i, free_list[i]->free.size, d);
			}
		}
	}
}


void dump_ring(RINGP r)
{
	if (debug_level)
	{
		LINEP	line;
		char		*txt;
			
		line = FIRST(r);
		while (!IS_TAIL(line))
		{
			txt = TEXT(line);
			debug("%s\n", txt);
			NEXT(line);
		}
	}
}
#endif
