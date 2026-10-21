/*
 * NewTOS patch of HypView hyp/hyp.c (freemint/hypview @ 6f3eeeb).
 *
 * Why (big-endian file format + LP64 struct layout): HypLoad() reads the
 * .HYP file header and index table with Fread straight into HYP_HEADER and
 * INDEX_ENTRY (hyp.h). On disk both are m68k structures: big-endian words
 * and longs, 2-byte alignment (header 12 bytes; index entry: length, type,
 * seek_offset at byte 2, comp_diff 6, next 8, previous 10, dir_index 12,
 * name at 14). On AArch64 the same structs are little-endian and padded
 * (sizeof(HYP_HEADER) 24, INDEX_ENTRY.seek_offset at byte 8), so every
 * field was misread. The extra-header tags/lengths and the ST-Guide flags
 * word are big-endian too.
 *
 * The patch decodes the 12-byte header from bytes, reads the raw index
 * table into a temporary block, and converts each entry into a native
 * INDEX_ENTRY (name copied behind it, entries 8-byte aligned) in the place
 * upstream keeps the table; the extra-header words are swapped after
 * reading. Every other line is upstream; changed places are marked NEWTOS.
 */
/*
 * HypView - (c)      - 2006 Philipp Donze
 *               2006 -      Philipp Donze & Odd Skancke
 *
 * A replacement hypertext viewer
 *
 * This file is part of HypView.
 *
 * HypView is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * HypView is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with HypView; if not, write to the Free Software
 * Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA  02111-1307  USA
 */

#ifdef __GNUC__
	#include <osbind.h>
	#include <fcntl.h>
#else
	#include <tos.h>
#endif
#include <string.h>
#include <stddef.h>	/* NEWTOS: offsetof */
#include <gemx.h>
#include "../diallib.h"
#include "../defs.h"
#include "../hyp.h"

/* NEWTOS: on-disk sizes and big-endian decoding */
#define HYP_FILE_HEADER_SIZE	12
#define HYP_FILE_IDX_NAME	14
#define HYP_NATIVE_IDX_SIZE(len) \
	(((offsetof(INDEX_ENTRY, name) + (len) - HYP_FILE_IDX_NAME) + 7) & ~(size_t)7)

static unsigned short
hyp_be16(const unsigned char *p)
{
	return (unsigned short)((p[0] << 8) | p[1]);
}

static unsigned long
hyp_be32(const unsigned char *p)
{
	return ((unsigned long)p[0] << 24) | ((unsigned long)p[1] << 16) |
		((unsigned long)p[2] << 8) | (unsigned long)p[3];
}

char *index_entry = "Index";
char *help_entry = "Help";

void HypClose(DOCUMENT *doc);
long HypGetNode(DOCUMENT *doc);
void HypGotoNode(DOCUMENT *doc, char *chapter,long node_num);

/*
 *		Ueberprueft ob es sich um einen Hypertext handelt und ladet
 *		danach die wichtigsten Daten in den Speicher.
 */
short
HypLoad(DOCUMENT *doc, short handle)
{
	long ret, memsize = sizeof(HYP_DOCUMENT), cache_size;
	HYP_HEADER head;
	HYP_DOCUMENT *hyp;
	INDEX_ENTRY *idxent;
	char *current_pos;
	unsigned short info[2], i;
	char *cptr;
	REF_FILE *ref;
	unsigned char head_raw[HYP_FILE_HEADER_SIZE];	/* NEWTOS */
	unsigned char *raw_idx;							/* NEWTOS */
	long native_idx_size;							/* NEWTOS */
	
	/*	Zurueck zum Dateianfang	*/
	Fseek(0, handle, 0);


	/*	Datei Extension ermitteln	*/
	cptr = strrchr(doc->filename,'.');
	if(cptr && !stricmp(cptr,".ref"))		/*	 REF Datei?	*/
	{
		ref = ref_load(handle);
		strcpy(cptr,".hyp");

		ret = Fopen(doc->path,O_RDONLY);
		if(ret < 0)
		{
			/*	Konnte die passende HYP Datei nicht geoeffnet werden?	*/
			strcpy(cptr,".ref");
			if(ref > 0)
				Mfree(ref);
			return(F_UNKNOWN);
		}
		handle = (short)ret;
	}
	else
		ref = NULL;

	if(!cptr || stricmp(cptr,".hyp"))	/*	 Keine .HYP Datei?	*/
	{
		if(ref > 0)
			Mfree(ref);
		return(F_UNKNOWN);
	}

	/*	Datei-Header einlesen	*/
	/* NEWTOS: was Fread(handle, sizeof(HYP_HEADER), &head) */
	ret = Fread(handle, HYP_FILE_HEADER_SIZE, head_raw);
	if (ret == HYP_FILE_HEADER_SIZE)
	{
		memcpy(head.magic, head_raw, 4);
		head.itable_size = (long)(int)hyp_be32(head_raw + 4);
		head.itable_num = hyp_be16(head_raw + 8);
		head.compiler_vers = head_raw[10];
		head.compiler_os = head_raw[11];
	}
	else if (ret >= 0)
		ret = -1;
	if(ret < 0)
	{
		/*	Fehler beim Laden der Daten...	*/
		Debug("Error %ld in Fread(%s)",ret,doc->filename);
		if(ref > 0)
			Mfree(ref);
		return(F_LOADERROR);
	}
	else if (head.magic[0] != 'H' ||
		head.magic[1] != 'D' ||
		head.magic[2] != 'O' ||
		head.magic[3] != 'C') /* 'HDOC' */
	{
		/*	"magischer Wert" nicht im Datei-Header...	*/
		Debug("ERROR: Wrong file format. Magic value not found.");
		if(ref > 0)
			Mfree(ref);
		return(F_UNKNOWN);
	}

/*
	printf("hyp-Header: Magic %4.4s itable_size %lu itable_num %u vers %d sys %d\n",
		&head.magic,head.itable_size,head.itable_num,head.compiler_vers,head.compiler_os);
*/

	if (head.itable_size < 0)
	{
		Debug("ERROR: negative size of index table size found!");
		if(ref > 0)
			Mfree(ref);
		return(F_UNKNOWN);
	}
	
	/* NEWTOS: read the raw (on-disk) index table now, to size the native one */
	raw_idx = (unsigned char *)Malloc(head.itable_size + 1);
	if (!raw_idx)
	{
		if(ref > 0)
			Mfree(ref);
		return (F_LOADERROR);
	}
	if (Fread(handle, head.itable_size, raw_idx) != head.itable_size)
	{
		Mfree(raw_idx);
		if(ref > 0)
			Mfree(ref);
		return (F_LOADERROR);
	}
	native_idx_size = 0;
	{
		long pos = 0;
		for(i = 0; i < head.itable_num; i++)
		{
			unsigned char len = raw_idx[pos];
			if (len < HYP_FILE_IDX_NAME || pos + len > head.itable_size)
			{
				Debug("ERROR: broken index table entry %u", i);
				Mfree(raw_idx);
				if(ref > 0)
					Mfree(ref);
				return(F_UNKNOWN);
			}
			native_idx_size += HYP_NATIVE_IDX_SIZE(len);
			pos += len;
		}
	}

	/*	Groesse der Index-Tabelle zur Speichermenge addieren	*/
	memsize += (head.itable_num + 1) * sizeof(INDEX_ENTRY *);
	/*	Groesse der Indexdaten zur Speichermenge addieren	*/
	memsize += native_idx_size;	/* NEWTOS: was head.itable_size */
	/*	Groesse der Cache-Tabelle zur Speichermenge addieren	*/
	cache_size = GetCacheSize(head.itable_num);
	memsize += cache_size;

	/*	Zum Anfang der Extra-Header	*/
	Fseek(head.itable_size + HYP_FILE_HEADER_SIZE, handle, 0);	/* NEWTOS: file header size */

	/*	Ermitteln der benoetigten Speichermenge fuer die Zusatzdaten	*/
	do
	{
		ret = Fread(handle, 4, info);	/*	Extra-Header-Info laden	*/
		info[0] = hyp_be16((unsigned char *)&info[0]);	/* NEWTOS */
		info[1] = hyp_be16((unsigned char *)&info[1]);	/* NEWTOS */
		if (info[0])
		{
			switch(info[0])
			{
				case 1:				/*	@database	*/
				case 3:				/*	@hostname	*/
				case 5:				/*	@author	*/
				case 6:				/*	@$VER	*/
					memsize += info[1];	/*	Speicher fuer diese Infos reservieren	*/
					break;
				case 2:				/*	@default	*/
				case 4:				/*	@options	*/
				case 7:				/*	@help	*/
				case 8:				/*	@subject	*/
				case 9:				/*	HypTree-Header	*/
				case 10:			/*	ST-Guide flags	*/
				case 11:			/*	@width	*/
					break;
			}
			Fseek(info[1],handle,1);
		}
	} while (info[0] && ret == 4);


	/*	Speicher	fuer Header und Indexdaten reservieren	*/
	hyp = (HYP_DOCUMENT *)Malloc(memsize);
	if(!hyp)
	{
		Mfree(raw_idx);	/* NEWTOS */
		Debug("Error while allocating %ld bytes.",memsize);
		if(ref > 0)
			Mfree(ref);
		return (F_LOADERROR);
	}
	hyp->num_index = head.itable_num;
	hyp->comp_vers = head.compiler_vers;
	hyp->comp_os = head.compiler_os;

	/*	Position der Index-Tabelle (=gerade nach dem Dokument-Kopf)	*/
	hyp->indextable = (INDEX_ENTRY **)((long)hyp + sizeof(HYP_DOCUMENT));

	/*	Zurueck zu den Indexdaten	*/
	/* NEWTOS: the raw table is already in raw_idx; continue after it */
	Fseek(HYP_FILE_HEADER_SIZE + head.itable_size, handle, 0);

	/*	Start der einzelnen Index-Eintrge	*/
	idxent = (INDEX_ENTRY *)(&hyp->indextable[head.itable_num+1]);

	/*	Startadresse der Cache-Tabelle setzen	*/
	hyp->cache = (void *)((long)idxent + native_idx_size);	/* NEWTOS: was head.itable_size */

	/*	Startadresse der restlichen Daten	*/
	current_pos = (char *)hyp->cache + cache_size;

	/*	Index-Tabelle generieren	*/
	{
		/* NEWTOS: convert each on-disk entry into a native INDEX_ENTRY */
		const unsigned char *raw = raw_idx;
		for(i = 0; i < head.itable_num; i++)
		{
			unsigned char len = raw[0];
			idxent->length = len;
			idxent->type = raw[1];
			idxent->seek_offset = hyp_be32(raw + 2);
			idxent->comp_diff = hyp_be16(raw + 6);
			idxent->next = hyp_be16(raw + 8);
			idxent->previous = hyp_be16(raw + 10);
			idxent->dir_index = hyp_be16(raw + 12);
			memcpy(&idxent->name, raw + HYP_FILE_IDX_NAME, len - HYP_FILE_IDX_NAME);
			hyp->indextable[i] = idxent;
			idxent = (INDEX_ENTRY *)((char *)idxent + HYP_NATIVE_IDX_SIZE(len));
			raw += len;
		}
		Mfree(raw_idx);
	}
	hyp->indextable[i] = NULL;

	/*	Cache initialisieren	*/
	InitCache(hyp);

	hyp->default_page = -1;

	/*	Seitennummer des Index ermitteln	*/
	hyp->index_page = find_nr_by_title(hyp, index_entry);

	/*	Seitennummer der Hilfeseite ermitteln	*/
	hyp->help_page = find_nr_by_title(hyp, help_entry);

	/*	Standard-Werte fuer die zusaetzlichen Infos	*/
	hyp->st_guide_flags = 0;
	hyp->line_width = 0;
	hyp->database = NULL;
	hyp->hostname = NULL;
	hyp->author = NULL;
	hyp->version = NULL;

	/* Extra-Headers (wenn noetig) laden...	*/
	do
	{
		short load_it;
		ret = Fread(handle, 4, info);	/*	Extra-Header-Info laden	*/
		info[0] = hyp_be16((unsigned char *)&info[0]);	/* NEWTOS */
		info[1] = hyp_be16((unsigned char *)&info[1]);	/* NEWTOS */
		load_it = TRUE;
		switch (info[0])
		{
			case 1:					/*	@database	*/
				hyp->database = current_pos;
				break;
			case 3:					/*	@hostname	*/
				hyp->hostname = current_pos;
				break;
			case 5:					/*	@author	*/
				hyp->author = current_pos;
				break;
			case 6:					/*	@$VER	*/
				hyp->version = current_pos;
				break;
			case 2:					/*	@default	*/
			{
				char temp[256];
				Fread(handle, info[1], temp);
				hyp->default_page = find_nr_by_title(hyp, temp);
				load_it = FALSE;
				break;
			}
			case 7:					/*	@help	*/
			{
				char temp[256];
				Fread(handle, info[1], temp);
				hyp->help_page = find_nr_by_title(hyp, temp);
				load_it = FALSE;
				break;
			}
			case 4:					/*	@options	*/
			case 8:					/*	@subject	*/
			case 9:					/*	HypTree-Header	*/
				Fseek(info[1], handle, 1);
				load_it = FALSE;
				break;
			case 10:					/*	ST-Guide flags	*/
			{
				if(info[1] != 2)		/*	Falsche Groesse?	*/
					Fseek(info[1], handle, 1);
				else
				{
					unsigned char f[2];	/* NEWTOS: big-endian word */
					Fread(handle, 2, f);
					hyp->st_guide_flags = (short)hyp_be16(f);
				}
				load_it = FALSE;
				break;
			}
			case 11:					/*	@width	*/
			{
				if(info[1] != 2)		/*	Falsche Groesse?	*/
					Fseek(info[1], handle, 1);
				else
				{
					short temp;
					Fread(handle, 2, &temp);
					hyp->line_width = *(unsigned char *)&temp;
				}
				load_it = FALSE;
				break;
			}
			default:
				load_it = FALSE;
		}
		if (load_it)
		{
			Fread(handle, info[1], current_pos);
			current_pos += info[1];
		}
	} while (info[0] && ret == 4);

	hyp->file = doc->path;
	hyp->entry = NULL;
	hyp->ref = ref;

	doc->data = hyp;
	doc->buttons = 0;
	doc->type = F_HYP;
	doc->displayProc = HypDisplayPage;
	doc->closeProc = HypClose;
	doc->gotoNodeProc = HypGotoNode;
	doc->getNodeProc = HypGetNode;
	doc->clickProc = HypClick;
	doc->autolocProc = HypAutolocator;
	doc->getCursorProc = HypGetCursorPosition;
	doc->blockProc = HypBlockOperations;

	doc->start_line = 0;
	doc->lines = 5;
	doc->columns = 60;
	
	if(ref)
	{
		Fclose(handle);
	}

	return(F_HYP);
}

void
HypClose(DOCUMENT *doc)
{
	HYP_DOCUMENT *hyp = doc->data;
	ClearCache(hyp);
	if(hyp->ref > 0)
		Mfree(hyp->ref);
	Mfree(hyp);
	doc->data = NULL;
}

long
HypGetNode(DOCUMENT *doc)
{
	HYP_DOCUMENT *hyp = doc->data;
	if (hyp->entry)
		return (hyp->entry->number);
	else
		Debug("Document %s has no open page",doc->filename);
	return(0);
}

long
HypFindNode(DOCUMENT *doc, char *chapter)
{
	HYP_DOCUMENT *hyp = doc->data;
	long node_num;
	/*	Zuerst im Hypertext suchen	*/
	node_num = find_nr_by_title(hyp,chapter);
	
	if (node_num < 0)
	{
		short line;
		if (!hyp->ref)				/* REF noch nicht geladen?	*/
		{
			char *cptr;
			char old_ext[5];
			long ret;

			cptr = strrchr(doc->filename, '.');
			strcpy(old_ext,cptr);
			strcpy(cptr, ".ref");

			ret = Fopen(doc->path,O_RDONLY);
			if(ret >= 0)
			{
				hyp->ref = ref_load((short)ret);
				Fclose((short)ret);
			}

			strcpy(cptr,old_ext);
		}
		
		if (ref_findnode(hyp->ref, chapter, &node_num, &line))
			doc->start_line = line;
	}
/*
	Debug("%s found at: node %ld  line %ld",chapter,node_num,doc->start_line);
*/	return(node_num);
}

void
HypGotoNode(DOCUMENT *doc, char *chapter, long node_num)
{
	HYP_DOCUMENT *hyp = doc->data;
	LOADED_NODE *node;

	/*	Kapitel am Namen identifizieren?	*/
	if (chapter && *chapter)
	{
		node_num=HypFindNode(doc,chapter);
	}
	else if (node_num < 0)	/*	Falls keine gueltige Nummer	*/
		node_num = hyp->default_page;

	if(node_num < 0)	/*	Falls keine gueltige Nummer	*/
	{
		node_num = 0;

		while (node_num < hyp->num_index)
		{
			if (hyp->indextable[node_num]->type <= POPUP)
				break;
			node_num++;
		}

		if (node_num >= hyp->num_index)
		{
			FileError(doc->filename,"no start page found.");
			return;
		}
	}
	else if (node_num >= hyp->num_index)		/*	Existiert kein solcher Eintrag?	*/
	{
		Debug("ERROR: No entry with number %d found in this document (max number is %d)",node_num,hyp->num_index-1);
		return;
	}

	if (hyp->indextable[node_num]->type > POPUP)
	{
		Debug("ERROR: Entry %d (Type: %d) is not a 'page' or a 'popup'",node_num,hyp->indextable[node_num]->type);
		return;
	}

	node = AskCache(hyp, node_num);
	
	if (!node)						/*	Seite noch nicht im Cache ?	*/
	{
		long size, loaded;
		void *data;
		size = GetDataSize(hyp, node_num);
	
		node = (LOADED_NODE *)Malloc(sizeof(LOADED_NODE) + size + 1);
		
		if(!node)
		{
			Debug("Error while allocating %ld bytes for node.\n",sizeof(LOADED_NODE)+size+1);
			return;
		}

		node->number = node_num;
		node->start = (char *)((long)node + sizeof(LOADED_NODE));
		node->end = &node->start[size];
		node->lines = 0;
		node->columns = 0;
		node->window_title = NULL;
		node->line_ptr = NULL;

		/*	Lade Eintrag in temp. Speicherbereich	*/
		data = LoadData(hyp, node_num, &loaded);
		if(!data)
		{
			Mfree(node);
			return;
		}

		/*	Daten vom temp. Speicher ans Ziel entpacken & entschluesseln	*/
		GetEntryBytes(hyp, node_num, data, node->start, size);
		Mfree(data);
		
		*node->end = 0;
		
		if(PrepareNode(hyp, node))
		{
			Debug("ERROR: while preparing page for display!");
			if(node->line_ptr)
				Mfree(node->line_ptr);
			Mfree(node);
			return;
		}
		TellCache(hyp,node_num,node);
	}

	hyp->entry = (LOADED_ENTRY *)node;

	/*	Node-Daten auf Dokument (=Fenster) bertragen	*/
	doc->lines = node->height / font_ch;
	doc->height = node->height;
	doc->columns = node->columns;
	doc->window_title = node->window_title;

	if(!doc->window_title)
		doc->window_title = &hyp->indextable[node_num]->name;

	/*	Toolbar anpassen	*/
	if(hyp->help_page >= 0)		/*	Existiert eine Hilfe Seite	*/
		doc->buttons |= BITVAL(TO_HELP);
	else
		doc->buttons &= ~BITVAL(TO_HELP);

	/*	Existiert ein Index ?	*/
	if(hyp->index_page >= 0)
		doc->buttons |= BITVAL(TO_INDEX);
	else
		doc->buttons &= ~BITVAL(TO_INDEX);

	if(node_num == hyp->indextable[node_num]->previous)
		doc->buttons &= ~BITVAL(TO_PREVIOUS);
	else
		doc->buttons |= BITVAL(TO_PREVIOUS);

	if(node_num == hyp->indextable[node_num]->next)
		doc->buttons &= ~BITVAL(TO_NEXT);
	else
		doc->buttons |= BITVAL(TO_NEXT);

	if(node_num == hyp->indextable[node_num]->dir_index)
		doc->buttons &= ~BITVAL(TO_HOME);
	else
		doc->buttons |= BITVAL(TO_HOME);

	if(HypCountExtRefs(node))
		doc->buttons |= BITVAL(TO_REFERENCES);
	else
		doc->buttons &= ~BITVAL(TO_REFERENCES);

	/*	ASCII Export supported	*/
	doc->buttons |= BITVAL(TO_SAVE);
}
