/*
 * NewTOS/AArch64 overlay (TASK/ZVIEW-M4, sdk/zview/build-zview.sh). The
 * pristine file is z-tools-04805b7/zview/av_prot.c (z-tools commit 04805b7).
 * ONE change, at upstream line 91 in va_start():
 *
 *   upstream:  char *p = *(char **) &evnt.buff[3];
 *   here:      p = (hi WORD evnt.buff[3] << 16) | lo WORD evnt.buff[4]
 *
 * Why: VA_START carries the command line as a 32-bit address, high WORD in
 * msg[3], low WORD in msg[4] (AV protocol; WinDom's own senders store it
 * that way, sdk/windom/overlay/src/shel_write.c, and docs/sdk/12-windom.md
 * section 3 records this convention). On the 68k the 4-byte load at
 * &buff[3] reads exactly that pair. On LP64 little-endian AArch64 it is an
 * 8-byte load over buff[3..6] with the low WORD first -- a garbage pointer.
 * User addresses are below 2 GiB on NewTOS (docs/sdk/05-abi-pitfalls.md A2),
 * so the 32-bit pair carries the whole pointer. The rest of va_start()
 * (ParseArgs, MenuBar, WindView, free) is unchanged.
 */
#include "general.h"
#include "pic_load.h"
#include "winimg.h"

/* Prototype */
void va_start( void);

/* taken from mxPlay - simplified version, takes only one argument */
static char* ParseArgs( char* cmdline )
{
	BOOL	inQuote = FALSE;
	int		i = 0;
	int		j = 0;
	char	all[1023+1];

	while( cmdline[i] != '\0' )
	{
		if( cmdline[i] != '\'' && cmdline[i] != ' ' && inQuote == FALSE )
		{
			/* simple file (not in quotes) */
			while( cmdline[i] != '\0' && cmdline[i] != ' ' )
			{
				all[j++] = cmdline[i++];
			}

			all[j++] = '\0';
			strcpy( cmdline, all );
			return cmdline;
		}

		switch( cmdline[i] )
		{
			case '\'':
				i++;
				if( inQuote == TRUE )
				{
					if( cmdline[i] == '\'' )					/* '' -> ' */
					{
						all[j++] = '\'';
						i++;
					}
					else
					{
						inQuote = FALSE;
						all[j++] = '\0';
						strcpy( cmdline, all );
						return cmdline;
					}
				}
				else
				{
					inQuote = TRUE;
				}
				break;

			case ' ':
				if( inQuote == TRUE )
				{
					all[j++] = cmdline[i++];
				}
				else
				{
					i++;
				}
				break;

			default:
				all[j++] = cmdline[i++];
				break;

		}
	}

	return cmdline;
}


/*==================================================================================*
 * void va_start:																	*
 *		Handle the VA_START message from the server.								*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		--																			*
 *----------------------------------------------------------------------------------*
 * returns: 																		*
 *      --																			*
 *==================================================================================*/

void va_start( void)
{
	char *p = (char *)(unsigned long)(((unsigned long)(unsigned short)evnt.buff[3] << 16) | (unsigned short)evnt.buff[4]);

	if( p)
	{
		p = ParseArgs( p );

		/* yes, I know, it's strange to reput the menubar here
		but if we don't do it, on MagiC, the menubar is not active after
		a VA_START :( */
		MenuBar( get_tree( MENU_BAR), 1);

		WindView( p);
		free( p);
	}
}
