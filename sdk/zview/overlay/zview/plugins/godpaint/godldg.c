/*
 * NewTOS/AArch64 overlay (TASK/ZVIEW-M4, sdk/zview/build-zview.sh). The
 * pristine file is z-tools-04805b7/zview/plugins/godpaint/godldg.c (z-tools
 * commit 04805b7). Three changes, all endianness:
 *
 *  1. reader: god_words_from_be() (defined after the GODHDR typedef,
 *     upstream lines 39-44) is called in reader_init() right after the file
 *     is read (after upstream line 88). A GodPaint file is nothing but
 *     16-bit big-endian words: the header ("G4" = 18228, or 1024), width,
 *     height, then one RGB565 word per pixel. Upstream reads them through
 *     GODHDR and, in reader_read() (upstream lines 134-139), through a
 *     uint16 pointer -- native m68k loads. On little-endian AArch64 each would
 *     come out byte-swapped (the header check at upstream line 90 would
 *     reject every file). Swapping the buffer's words once in place makes
 *     every upstream read see the m68k value; reader_read() is unchanged.
 *  2. encoder_init(): upstream lines 225-226 write width and height with
 *     Fwrite(&info->width, 2) -- the native in-memory bytes; here they are
 *     written big-endian, as the format (and the reader) has them.
 *  3. encoder_write(): upstream line 266 stores each RGB565 pixel word
 *     natively before the Fwrite; here it is stored big-endian. The colour
 *     arithmetic is unchanged.
 *
 * zView itself depends on this codec: its browser icons are GodPaint files
 * (catalog/catalog_icons.c:124-129, icons\*.god).
 */
#include "zview.h"

boolean CDECL reader_init( const char *name, IMGINFO info);
boolean CDECL reader_read( IMGINFO info, uint8 *buffer);
void    CDECL reader_get_txt( IMGINFO info, txt_data *txtdata);
void    CDECL reader_quit( IMGINFO info);
boolean CDECL encoder_init( const char *name, IMGINFO info);
boolean CDECL encoder_write( IMGINFO info, uint8 *buffer);
void 	CDECL encoder_quit( IMGINFO info);
void	CDECL init( void);


PROC DEGASFunc[] = 
{
	{ "plugin_init", 	"", init},
	{ "reader_init", 	"", reader_init},
	{ "reader_get_txt", "", reader_get_txt},
	{ "reader_read", 	"", reader_read},
	{ "reader_quit", 	"", reader_quit},
	{ "encoder_init", 	"", encoder_init},
	{ "encoder_write",	"", encoder_write},
	{ "encoder_quit", 	"", encoder_quit}
};


LDGLIB godpaint_plugin =
{
	0x0200, 			/* Plugin version */
	8,					/* Number of plugin's functions */
	DEGASFunc,			/* List of functions */
	"GOD",				/* File's type Handled */
	LDG_NOT_SHARED, 	/* The flags NOT_SHARED is used here.. even if zview plugins are reentrant 
					   	   and are shareable, we must use this flags because we don't know if the 
					   	   user has ldg.prg deamon installed on his computer */
	libshare_exit,				/* Function called when the plugin is unloaded */
	1L					/* Howmany file type are supported by this plugin */
};

typedef struct 
{
	int16 header;
	int16 width;
	int16 height;
} GODHDR;

/* NewTOS overlay: see the header of this file. */
static void god_words_from_be( void *buf, int32 size)
{
	uint8	*b = ( uint8*)buf;
	uint8	t;
	int32	i;

	for( i = 0; i + 1 < size; i += 2)
	{
		t 		 = b[i];
		b[i] 	 = b[i + 1];
		b[i + 1] = t;
	}
}



/*==================================================================================*
 * boolean CDECL reader_init:														*
 *		Open the file "name", fit the "info" struct. ( see zview.h) and make others	*
 *		things needed by the decoder.												*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		name		->	The file to open.											*
 *		info		->	The IMGINFO struct. to fit.									*
 *----------------------------------------------------------------------------------*
 * return:	 																		*
 *      TRUE if all ok else FALSE.													*
 *==================================================================================*/
boolean CDECL reader_init( const char *name, IMGINFO info)
{
	int16 		handle;
	int32		file_size;
	GODHDR		*god;

	if ( ( handle = ( int16)Fopen( name, 0)) < 0)
		return FALSE;

	file_size = Fseek( 0L, handle, 2);

	Fseek( 0L, handle, 0);

	god = ( GODHDR *)malloc( file_size);

	if ( god == NULL)
	{
		Fclose( handle);
		return FALSE;	
	}

	if ( Fread( handle, file_size, god) != file_size)
	{
		free( god);
		Fclose( handle);
		return FALSE;	
	}

	Fclose( handle);

	god_words_from_be( god, file_size);

	if( ( god->header != 1024) /* some beta version of godpaint? I don't know but some files have this header */
	&& ( god->header != 18228   /* G4 */))
	{
		free( god);
		return FALSE;	
	}

	info->planes 					= 16;
	info->real_width  = info->width = god->width;
	info->real_height = info->height= god->height;
	info->components				= 3;
	info->indexed_color				= FALSE;
	info->colors  					= 1L << info->planes;
	info->memory_alloc 				= TT_RAM;
	info->page	 					= 1;
	info->delay						= 0;
	info->orientation				= UP_TO_DOWN;
	info->num_comments				= 0;
	info->max_comments_length 		= 0;
	info->_priv_ptr					= ( void*)god;
	info->_priv_var_more			= 3;		
	info->_priv_ptr_more			= NULL;

	strcpy( info->info, "Reservoir Gods - Godpaint");	
	strcpy( info->compression, "None");

	return TRUE;
}


/*==================================================================================*
 * boolean CDECL reader_read:														*
 *		This function fits the buffer with image data								*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		buffer		->	The destination buffer.										*
 *		info		->	The IMGINFO struct. to fit.									*
 *----------------------------------------------------------------------------------*
 * return:	 																		*
 *      TRUE if all ok else FALSE.													*
 *==================================================================================*/
boolean CDECL reader_read( IMGINFO info, uint8 *buffer)
{
	GODHDR		*god	= ( GODHDR*)info->_priv_ptr;
	uint16 		*source = ( uint16*)god + info->_priv_var_more;
	int16		i, ii;

	for( i = 0, ii = 0; i < info->width; i++)
	{
		register uint16 src16 = source[i];

		buffer[ii++] = (( src16 >> 11) & 0x001F) << 3; 
        buffer[ii++] = (( src16 >> 5)  & 0x003F) << 2;
        buffer[ii++] = (( src16)       & 0x001F) << 3;
	}

	info->_priv_var_more += info->width;
	return TRUE;
}


/*==================================================================================*
 * boolean CDECL reader_get_txt														*
 *		This function , like other CDECL function must be always present.			*
 *		It fills txtdata struct. with the text present in the picture ( if any).	*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		txtdata		->	The destination text buffer.								*
 *		info		->	The IMGINFO struct. to fit.									*
 *----------------------------------------------------------------------------------*
 * return:	 																		*
 *      --																			*
 *==================================================================================*/
void CDECL reader_get_txt( IMGINFO info, txt_data *txtdata)
{
	return;
}


/*==================================================================================*
 * boolean CDECL reader_quit:														*
 *		This function makes the last job like close the file handler and free the	*
 *		allocated memory.															*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		info		->	The IMGINFO struct. to fit.									*
 *----------------------------------------------------------------------------------*
 * return:	 																		*
 *      --																			*
 *==================================================================================*/
void CDECL reader_quit( IMGINFO info)
{
	GODHDR		*god	= ( GODHDR*)info->_priv_ptr;

	free( god);
}


/*==================================================================================*
 * boolean CDECL encoder_init:														*
 *		Open the file "name", fit the "info" struct. ( see zview.h) and make others	*
 *		things needed by the encoder.												*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		name		->	The file to open.											*
 *		info		->	The IMGINFO struct. to fit.									*
 *----------------------------------------------------------------------------------*
 * return:	 																		*
 *      TRUE if all ok else FALSE.													*
 *==================================================================================*/
boolean CDECL encoder_init( const char *name, IMGINFO info)
{	
	uint16 		*line_buffer = NULL;		   
	int8		header_id[2] = "G4";
	int32 		file;

	if ( ( file = Fcreate( name, 0)) < 0)
		return FALSE;

	line_buffer	= ( uint16*) malloc( ( info->width + 1) << 1);

	if ( line_buffer == NULL) 
	{
		Fclose( file);
		return FALSE;
	}

	/* we test only the first Fwrite.. if it's ok, the others *should* works also */
	if( Fwrite( file, 2, header_id) != 2)
	{
		free( line_buffer);
		Fclose( file);
		return FALSE;
	}	

	{
		uint8 be[4];

		be[0] = ( uint8)( info->width >> 8);
		be[1] = ( uint8)info->width;
		be[2] = ( uint8)( info->height >> 8);
		be[3] = ( uint8)info->height;
		Fwrite( file, 2, ( void*)&be[0]);
		Fwrite( file, 2, ( void*)&be[2]);
	}	


	info->planes   			= 24;	/* the wanted input data format.. currently Zview return only 24 bits data, so we make the convertion to 16 bits ourself */
	info->components 		= 3;
	info->colors  			= 16777215L;
	info->orientation		= UP_TO_DOWN;
	info->indexed_color	 	= FALSE;
	info->page			 	= 1;
	info->delay 		 	= 0;
	info->_priv_ptr	 		= line_buffer;
	info->_priv_var	 		= file;
	info->_priv_ptr_more	= NULL;				
	info->__priv_ptr_more	= NULL;	

	return TRUE;
}


/*==================================================================================*
 * boolean CDECLencoder_write:														*
 *		This function write data from buffer to file								*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		buffer		->	The source buffer.											*
 *		info		->	The IMGINFO struct		.									*
 *----------------------------------------------------------------------------------*
 * return:	 																		*
 *      TRUE if all ok else FALSE.													*
 *==================================================================================*/
boolean CDECL encoder_write( IMGINFO info, uint8 *buffer)
{
	uint16 *source = ( uint16*)info->_priv_ptr;
	int32  byte_to_write = info->width << 1;
	uint16  ii, i;

	for( ii = i = 0; i < info->width; i++, ii += 3)
	{
		register uint8 *rgb = &buffer[ii];

		register uint16 v = (((uint16)rgb[0] & 0xF8) << 8) | (((uint16)rgb[1] & 0xFC) << 3) | ( rgb[2] >> 3);

		(( uint8*)&source[i])[0] = ( uint8)( v >> 8);
		(( uint8*)&source[i])[1] = ( uint8)v;
	}

	if( Fwrite( info->_priv_var, byte_to_write, ( uint8*)source) != byte_to_write)
		return FALSE;

	return TRUE;
}



/*==================================================================================*
 * boolean CDECL encoder_quit:														*
 *		This function makes the last job like close the file handler and free the	*
 *		allocated memory.															*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		info		->	The IMGINFO struct. to fit.									*
 *----------------------------------------------------------------------------------*
 * return:	 																		*
 *      --																			*
 *==================================================================================*/
void CDECL encoder_quit( IMGINFO info)
{
	uint8 *buffer = ( uint8*)info->_priv_ptr;

	free( buffer);
	Fclose( info->_priv_var);
}




/*==================================================================================*
 * boolean CDECL init:																*
 *		First function called from zview, in this one, you can make some internal	*
 *		initialisation.																*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		--																			*
 *----------------------------------------------------------------------------------*
 * return:	 																		*
 *      --																			*
 *==================================================================================*/
void CDECL init( void)
{
	libshare_init();
}


/*==================================================================================*
 * int main:																		*
 *		Main function, his job is to call ldg_init function.						*
 *----------------------------------------------------------------------------------*
 * input:																			*
 *		--																			*
 *----------------------------------------------------------------------------------*
 * return:	 																		*
 *      0																			*
 *==================================================================================*/
int main( void)
{
	ldg_init( &godpaint_plugin);
	return( 0);
}
