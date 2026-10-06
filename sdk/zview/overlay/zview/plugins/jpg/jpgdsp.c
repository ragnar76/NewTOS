/*
 * NewTOS/AArch64 overlay (TASK/ZVIEW-M4, sdk/zview/build-zview.sh). The
 * pristine file is z-tools-04805b7/zview/plugins/jpg/jpgdsp.c (z-tools
 * commit 04805b7). The one m68k-assembly retarget of the JPEG codec (the
 * project's "m68k asm gets an AArch64 equivalent" exception):
 *
 *   upstream lines 10-88: five GNU statement-expression macros
 *   JPGDOpenDriver/CloseDriver/GetImageInfo/GetImageSize/DecodeImage, each
 *   m68k inline assembly that loads the JPGD_STRUCT pointer into a0, fetches
 *   the routine from the driver table at byte offset 4/8/16/20/24 and calls
 *   it with jsr, taking the result from d0 -- the register convention of
 *   Brainstorm's DSP JPEG decoder (jpgdh.h, "JPEGD header file", cookie
 *   _JPD).
 *   here: the same five names call the same table entries
 *   (JPGDDRV_STRUCT members JPGDOpenDriver ... JPGDDecodeImage, jpgdh.h,
 *   which are the entries at those m68k offsets) as ordinary C function
 *   pointers -- the argument in x0 and the result in x0 per AAPCS64, the
 *   only calling convention a native AArch64 driver behind such a cookie
 *   could have. The result is narrowed to int32 like the d0 value it
 *   replaces.
 *
 * Reachability on NewTOS: reader_dsp_init() runs only when the codec's
 * init() found a _JPD cookie (jpgldg.c:774); NewTOS publishes none, so this
 * path is compiled and linked (jpgldg.c references reader_dsp_init, jpgdrv
 * and dsp_ram) but not executed. The rest of the file is unchanged.
 */
#include "zview.h"
#include "jpgdh.h"

int16 dsp_ram = 0;

int16 reader_dsp_init( const char *name, IMGINFO info);

JPGDDRV_PTR jpgdrv = NULL;

/* NewTOS overlay: the five m68k register-call wrappers of upstream lines
   10-88, as AAPCS64 calls through the driver table (see the file header). */
#define	JPGDOpenDriver( jpgd, jpgdrv)		(( int32)( jpgdrv)->JPGDOpenDriver( jpgd))
#define	JPGDCloseDriver( jpgd, jpgdrv)		(( int32)( jpgdrv)->JPGDCloseDriver( jpgd))
#define	JPGDGetImageInfo( jpgd, jpgdrv)		(( int32)( jpgdrv)->JPGDGetImageInfo( jpgd))
#define	JPGDGetImageSize( jpgd, jpgdrv)		(( int32)( jpgdrv)->JPGDGetImageSize( jpgd))
#define	JPGDDecodeImage( jpgd, jpgdrv)		(( int32)( jpgdrv)->JPGDDecodeImage( jpgd))

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
int16 reader_dsp_init( const char *name, IMGINFO info)
{
	char		pad[] = { -1, -1, -1, -1, -1, -1, -1, -1, 0, 0};
	void		*src, *dst;
	int16		jpeg_file;
	int32		jpgdsize, jpeg_file_size;
	JPGD_PTR 	jpgd;

	if ( ( jpeg_file = ( int16)Fopen( name, 0)) < 0)
		return GOLBAL_ERROR;

	jpeg_file_size = Fseek( 0L, jpeg_file, 2);

	Fseek( 0L, jpeg_file, 0);

	if (( src = ( void*)Mxalloc( jpeg_file_size + sizeof( pad), dsp_ram)) == NULL)
	{
		Fclose( jpeg_file);
		return GOLBAL_ERROR;	
	}

	if ( Fread( jpeg_file, jpeg_file_size, src) != jpeg_file_size)
	{
		Mfree( src);
		Fclose( jpeg_file);
		return GOLBAL_ERROR;	
	}

	Fclose( jpeg_file);

	memcpy( ( uint8 *)src + jpeg_file_size, pad, sizeof( pad));

	jpgdsize = jpgdrv->JPGDGetStructSize();

	if( jpgdsize < 1)
	{
		Mfree( src);
		return DSP_ERROR;
	}	   
	   
	jpgd = ( JPGD_PTR)Mxalloc( jpgdsize, dsp_ram);

	if( jpgd == NULL)
	{
		Mfree( src);
		return GOLBAL_ERROR;
	}

	memset( ( void *)jpgd, 0, jpgdsize);

	if( JPGDOpenDriver( jpgd, jpgdrv) != 0)
	{
		Dsp_Unlock();
		Mfree( jpgd);
		Mfree( src);
		return DSP_ERROR;
	}

	jpgd->InPointer = src;
	jpgd->InSize    = jpeg_file_size;

	if( JPGDGetImageInfo( jpgd, jpgdrv) != 0)
	{
		JPGDCloseDriver( jpgd, jpgdrv);
		Dsp_Unlock();
		Mfree( jpgd);
		Mfree( src);
		return DSP_ERROR;
	}

	jpgd->OutComponents = 3;
	jpgd->OutPixelSize  = 3;

	if( JPGDGetImageSize( jpgd, jpgdrv) != 0)
	{
		JPGDCloseDriver( jpgd, jpgdrv);
		Mfree( jpgd);
		Mfree( src);
		return DSP_ERROR;
	}

	if(( dst = ( void*)Mxalloc( jpgd->OutSize, dsp_ram)) == NULL)
	{
		JPGDCloseDriver( jpgd, jpgdrv);
		Mfree( jpgd);
		Mfree( src);
		return GOLBAL_ERROR;
	}

	jpgd->OutPointer  = dst;
	jpgd->OutFlag     = 0;			 
	jpgd->UserRoutine = NULL;

	if( JPGDDecodeImage( jpgd, jpgdrv) != 0)
	{
		JPGDCloseDriver( jpgd, jpgdrv);
		Mfree( jpgd);
		Mfree( src);
		Mfree( dst);
		return DSP_ERROR;
	}

	info->components 			= 3;
	info->width   				= jpgd->MFDBStruct.fd_w;
	info->height  				= jpgd->MFDBStruct.fd_h;
	info->real_width			= info->width;
	info->real_height			= info->height;
	info->memory_alloc 			= TT_RAM;
	info->planes   				= 24;
	info->orientation 			= UP_TO_DOWN;
	info->colors  				= 1uL << ( uint32)info->planes;
	info->indexed_color 		= FALSE;
	info->page	 				= 1;
	info->delay		 			= 0;
	info->num_comments			= 0;
	info->max_comments_length	= 0;

	info->_priv_ptr				= dst;	
	info->_priv_var				= jpgd->MFDBStruct.fd_wdwidth << 1;
	info->_priv_var_more		= info->_priv_var;

	strcpy( info->info, "JPEG");
	strcpy( info->compression, "JPG");	

	JPGDCloseDriver( jpgd, jpgdrv);

	Mfree( jpgd);
	Mfree( src);

	return ALL_OK;
}

