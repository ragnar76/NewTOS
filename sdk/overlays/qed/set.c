#include "global.h"
#include "set.h"

#include <string.h>	/* memcpy / memset -- mintlib */

/*
 * ==========================================================================
 *  Word-width- and endianness-neutral overlay of QED's set module
 *  (src/set.c).
 *
 *  WHY AN OVERLAY IS NEEDED
 *  ------------------------
 *  types.h:13-16 declares the set as
 *        #define SETSIZE 32
 *        #define SETMAX  (SETSIZE*32 - 1)      ... 1023
 *        typedef unsigned long SET[SETSIZE];
 *  i.e. "32 longs, 32 bits each, 1024 element numbers".  That identity holds
 *  on m68k, where sizeof(unsigned long) == 4 AND the machine is big-endian.
 *  On AArch64/LP64 neither half holds: a long is 8 bytes, and the byte order
 *  is little-endian.
 *
 *  src/set.c addresses the very same bits in TWO different ways:
 *    a) BYTE-addressed, and therefore already neutral --
 *       setincl (:161), setexcl (:169), setchg (:177), setin (:185) use
 *       byte (elt>>3) and mask (0x80 >> (elt&7)).
 *    b) WORD-addressed with a HARDCODED 32 --
 *       setfree (:8), setmax (:30), setmin (:47) step the element counter in
 *       units of 32 per *set++ and use the constants 0x80000000L /
 *       0xFFFFFFFFL as "the top bit of a word"; setall (:77) and setnot (:105)
 *       write/complement 0xFFFFFFFFL per word.
 *  On m68k (a) and (b) describe the same bit for the same element number.
 *  On LP64 little-endian they do not, and the two halves of the module stop
 *  talking about the same set.
 *
 *  Measured on this data model with the pristine file (see the settest
 *  harness quoted in the receipt): after setincl(s,1) the raw word[0] reads
 *  0x0000000000000040, setmin and setmax both answer 25 instead of 1, and the
 *  loop in QED's restore_edit (edit.c:2106-2109)
 *        max = setmax(chg_links);
 *        for (link = setmin(chg_links); link <= max; link++)
 *            if (setin(chg_links, link)) ...
 *  never enters its body for any window handle except 0 -- so every
 *  incremental screen update QED makes (a typed character, the '*' in the
 *  title, the line/column info line) is computed and then thrown away at
 *  edit.c:2145-2147.
 *
 *  THE BIT NUMBERING CHOSEN, AND WHY
 *  ---------------------------------
 *  Element e lives in byte (e >> 3) of the SET object, in mask (0x80 >> (e&7)):
 *  a FLAT byte numbering, identical on every word width and byte order.
 *
 *  This is not a new convention, it is the one the pristine module already
 *  uses in setincl/setexcl/setchg/setin, and on m68k it is bit-for-bit the
 *  same as the word numbering of setmin/setmax/setfree: a big-endian 32-bit
 *  word holds element w*32 in its MSB and w*32+31 in its LSB, which is byte
 *  w*4 bit 0x80 .. byte w*4+3 bit 0x01.  Consequences:
 *    - The first (SETMAX+1)/8 == 128 bytes of a SET carry a BYTE-IDENTICAL
 *      image to the one the same program builds on m68k.  Any on-disk or
 *      cross-checked set image keeps its meaning.
 *    - setincl/setexcl/setchg/setin stay VERBATIM: four of the seventeen
 *      functions need no change at all, which is the strongest argument for
 *      this numbering over re-mapping the element onto "the low 32 bits of
 *      each 64-bit long" (the alternative offered in the task).  That
 *      alternative would have had to rewrite those four too, would waste the
 *      upper half of every word, and would produce an image that no longer
 *      matches m68k.
 *    - types.h is vendored source and is NOT touched, so SETMAX stays 1023
 *      and exactly 1024 element numbers exist -- unchanged from m68k.  On
 *      LP64 the SET object is SETSIZE*sizeof(unsigned long) == 256 bytes, so
 *      its upper 128 bytes are PADDING: never read, and normalised to zero by
 *      every function here that writes a whole set.  Widening the module to
 *      the 2048 bits the object would hold is impossible anyway, because
 *      setincl/setin reject elt > SETMAX and SETMAX lives in the file we may
 *      not edit.  No caller is affected: SETMAX appears nowhere outside
 *      set.c/types.h, and the largest element QED ever inserts is a window
 *      handle (edit.c:2215), a project line number (projekt.c:594) or a
 *      character code 0..255 (str2set).
 *
 *  WHAT CHANGED, FUNCTION BY FUNCTION (17 in set.h)
 *  ------------------------------------------------
 *    setfree, setmax, setmin  rewritten byte-wise.  Return values including
 *                             the degenerate ones are kept exactly: setmax of
 *                             the empty set is -1, setmin of the empty set and
 *                             setfree of the full set are SETMAX+1 == 1024,
 *                             which is what the m68k loops leave in `i`.
 *    setall, setnot           rewritten byte-wise; on LP64 the pristine
 *                             0xFFFFFFFFL only ever reached half of each word.
 *    setclr, setand, setor,   kept word-wise semantics but expressed over the
 *    setxor, setcpy, setcmp   byte image, so they cover exactly the
 *                             addressable region on any width.
 *    setincl, setexcl,        VERBATIM (byte-addressed already); only the
 *    setchg, setin, setcard,  bits[] table is typed unsigned char instead of
 *    str2set                  char, which is what -funsigned-char already made
 *                             it.  setcard is correct as soon as setmin/setmax
 *                             are, and keeps its original body on purpose.
 *
 *  The receipt's harness compares all seventeen, for every element 0..SETMAX
 *  and for randomised, paired and range sets, against a literal
 *  transliteration of the pristine source running on big-endian 32-bit words.
 * ==========================================================================
 */

/*
 * The addressable region: one bit per element number, 0..SETMAX.
 * 128 bytes -- on m68k that is the whole object, on LP64 its first half.
 */
#define SET_BYTES	((short)((SETMAX + 1) / 8))
/* The whole object, however wide a long happens to be. */
#define SET_RAW		((short)(SETSIZE * sizeof(unsigned long)))

static const unsigned char bits[] = {128, 64, 32, 16, 8, 4, 2, 1};

/*
 * Keep the bytes past the last addressable element deterministic, so that a
 * SET is fully defined by its elements on this data model too (callers such as
 * projekt.c:1360 compare whole sets, and options.c:498 memcpy()s structs that
 * embed them).  On m68k SET_RAW == SET_BYTES and this is a no-op.
 */
static void setpad(SET set)
{
	if (SET_RAW > SET_BYTES)
		memset((unsigned char *)set + SET_BYTES, 0,
		       (size_t)(SET_RAW - SET_BYTES));
}

/*****************************************************************************/

short setfree(SET set)			/* smallest element NOT in the set */
{
	const unsigned char *p = (const unsigned char *)set;
	short	b, i;

	for (b = 0; b < SET_BYTES; b++)
		if (p[b] != 0xFF)
			for (i = 0; i < 8; i++)
				if ((p[b] & bits[i]) == 0)
					return (short)(b * 8 + i);
	return (short)(SETMAX + 1);
}

/*****************************************************************************/

short setmax(SET set)			/* greatest element in the set, -1 if empty */
{
	const unsigned char *p = (const unsigned char *)set;
	short	b, i;

	for (b = SET_BYTES; (--b) >= 0; )
		if (p[b] != 0)
			for (i = 7; i >= 0; i--)
				if (p[b] & bits[i])
					return (short)(b * 8 + i);
	return -1;
}

/*****************************************************************************/

short setmin(SET set)			/* smallest element in the set */
{
	const unsigned char *p = (const unsigned char *)set;
	short	b, i;

	for (b = 0; b < SET_BYTES; b++)
		if (p[b] != 0)
			for (i = 0; i < 8; i++)
				if (p[b] & bits[i])
					return (short)(b * 8 + i);
	return (short)(SETMAX + 1);
}

/*****************************************************************************/

void setcpy (SET set1, SET set2)
{
	memcpy(set1, set2, (size_t)SET_RAW);
}

/*****************************************************************************/

void setall (SET set)
{
	memset(set, 0xFF, (size_t)SET_BYTES);
	setpad(set);
}

/*****************************************************************************/

void setclr (SET set)
{
	memset(set, 0, (size_t)SET_RAW);
}

/*****************************************************************************/

void setnot (SET set)
{
	unsigned char *p = (unsigned char *)set;
	short	i;

	for (i = 0; i < SET_BYTES; i++)
		p[i] = (unsigned char)(p[i] ^ 0xFF);
	setpad(set);
}

/*****************************************************************************/

void setand (SET set1, SET set2)
{
	unsigned char		*p1 = (unsigned char *)set1;
	const unsigned char	*p2 = (const unsigned char *)set2;
	short	i;

	for (i = 0; i < SET_BYTES; i++)
		p1[i] = (unsigned char)(p1[i] & p2[i]);
	setpad(set1);
}

/*****************************************************************************/

void setor (SET set1, SET set2)
{
	unsigned char		*p1 = (unsigned char *)set1;
	const unsigned char	*p2 = (const unsigned char *)set2;
	short	i;

	for (i = 0; i < SET_BYTES; i++)
		p1[i] = (unsigned char)(p1[i] | p2[i]);
	setpad(set1);
}

/*****************************************************************************/

void setxor (SET set1, SET set2)
{
	unsigned char		*p1 = (unsigned char *)set1;
	const unsigned char	*p2 = (const unsigned char *)set2;
	short	i;

	for (i = 0; i < SET_BYTES; i++)
		p1[i] = (unsigned char)(p1[i] ^ p2[i]);
	setpad(set1);
}

/*****************************************************************************/

void setincl (SET set, short elt)
{
	if (elt >= 0 && elt <= SETMAX)
		((unsigned char *)set)[elt >> 3] |= bits[elt & 7];
}

/*****************************************************************************/

void setexcl (SET set, short elt)
{
	if (elt >= 0 && elt <= SETMAX)
		((unsigned char *)set)[elt >> 3] &= (unsigned char)(~bits[elt & 7]);
}

/*****************************************************************************/

void setchg (SET set, short elt)
{
	if (elt >= 0 && elt <= SETMAX)
		((unsigned char *)set)[elt >> 3] ^= bits[elt & 7];
}

/*****************************************************************************/

bool setin (SET set, short elt)
{
	if (elt >= 0 && elt <= SETMAX)
		return ((((const unsigned char *)set)[elt >> 3] & bits[elt & 7])
		        ? TRUE : FALSE);
	else
		return (FALSE);
}

/*****************************************************************************/

/*
 * Compares the addressable region only.  The pristine version compared
 * SETSIZE longs, which on m68k IS the addressable region; on LP64 that would
 * additionally compare the padding, i.e. bits no element number can name.
 */
bool setcmp (SET set1, SET set2)
{
	const unsigned char	*p1 = (const unsigned char *)set1;
	const unsigned char	*p2 = (const unsigned char *)set2;
	short	i;

	if (set2 == NULL)
		for (i = 0; i < SET_BYTES && p1[i] == 0; i++) ;
	else
		for (i = 0; i < SET_BYTES && p1[i] == p2[i]; i++) ;
	return (i == SET_BYTES);
}

/*****************************************************************************/

/* Verbatim from src/set.c:208 -- correct as soon as setmin/setmax are. */
short setcard (SET set)
{
	short i, card, max;

	max = setmax(set);
	for (i=setmin(set), card=0; i<=max; i++)
		if (setin (set, i)) card++;

	return (card);
}

/*****************************************************************************/

/* Verbatim from src/set.c:221 -- byte-addressed through setincl/setclr. */
void str2set (char *str, SET set)
{
	short	i;

	setclr(set);
	i = 0;
	while(str[i])
	{
		if (str[i]=='-' && i > 0)
		{
			char c;

			i++;
			for (c = str[i - 2]; c < str[i]; c++)
				setincl(set,c);
		}
		else
			setincl(set,str[i++]);
	}
}
