/*
 * jpgref.c -- host reference decoder for measurement-2026-10-06-zview-m4:
 * decodes a JPEG with the PINNED IJG 9f sources (sdk/libjpeg/jpeg-9f, built
 * for the host by jpgref.sh) using exactly the settings of zView's JPEG
 * codec, z-tools-04805b7/zview/plugins/jpg/jpgldg.c reader_init():
 * jpeg_stdio_src, jpeg_read_header(TRUE), dct_method = JDCT_IFAST,
 * do_fancy_upsampling = FALSE (:437-439), out_color_space RGB, one scanline
 * per call (:526). Writes <w uint16 LE><h uint16 LE><RGB...> to argv[2]
 * WITHOUT the screen quantisation (check-shot.py applies it).
 */
#include <stdio.h>
#include <stdlib.h>
#include <jpeglib.h>

int main(int argc, char **argv)
{
	struct jpeg_decompress_struct d;
	struct jpeg_error_mgr e;
	FILE *in, *out;
	JSAMPROW row;

	if (argc != 3 || !(in = fopen(argv[1], "rb")) || !(out = fopen(argv[2], "wb")))
		return 2;
	d.err = jpeg_std_error(&e);
	jpeg_create_decompress(&d);
	jpeg_stdio_src(&d, in);
	jpeg_read_header(&d, TRUE);
	d.dct_method = JDCT_IFAST;
	d.do_fancy_upsampling = FALSE;
	jpeg_start_decompress(&d);
	if (d.out_color_space != JCS_RGB || d.output_components != 3)
		return 3;
	fputc(d.output_width & 0xFF, out); fputc(d.output_width >> 8, out);
	fputc(d.output_height & 0xFF, out); fputc(d.output_height >> 8, out);
	row = malloc(d.output_width * 3);
	while (d.output_scanline < d.output_height) {
		jpeg_read_scanlines(&d, &row, 1);
		fwrite(row, 3, d.output_width, out);
	}
	jpeg_finish_decompress(&d);
	jpeg_destroy_decompress(&d);
	fclose(in); fclose(out);
	return 0;
}
