/*
 * test_casefile.c -- the host-side case table reader, per area.
 *
 * The reader takes three things from a table: its CRC, how many cases it
 * holds, and the comment column so a difference report can name a case. The
 * first two are area-agnostic -- they come from the shared line iterator and
 * the shared prelude rule -- but the comment column is NOT: it is column five
 * in the VDI form and column seven or eight in the AES form, and no amount of
 * looking at a line settles WHICH FORM it is, because a VDI comment may itself
 * hold tabs.
 *
 * So the area is an argument, and this test is what says so.  The AES column
 * count, in contrast, is in the file -- its "!columns" directive -- and the
 * reader has to honour it; the eight column test below is what says that.
 */
#include "tdtest.h"
#include "casefile.h"

static const char *vdi_text =
	"# a VDI table\n"
	"6\t0\t-\t0,0,10,10\tv_pline two points\n"
	"+21\t0\t2\t-\tvst_font 2, a prelude\n"
	"38\t0\t-\t-\tvqt_attributes\ttabs\tkept\n";

static const char *aes_text =
	"# an AES table\n"
	"108\t6\t5\t0\t0\t1,0x0FEF,100,100,200,150\twind_calc WC_WORK\n"
	"+77\t0\t5\t0\t0\t-\tgraf_handle, a prelude\n"
	"104\t2\t5\t0\t0\t0,4\twind_get 0 WF_WXYWH\ttabs\tkept\n";

static void write_file(const char *path, const char *text)
{
	FILE *f = fopen(path, "wb");

	TD_CHECK(f != NULL);
	if (!f)
		return;
	TD_CHECK_EQ(fwrite(text, 1, strlen(text), f), strlen(text));
	fclose(f);
}

static void check_vdi(const char *path)
{
	struct td_casefile cf;
	const char *why = NULL;

	printf("casefile: the VDI form takes the comment from column five\n");

	write_file(path, vdi_text);
	TD_CHECK_EQ(td_casefile_read(&cf, path, TD_AREA_VDI, &why), 0);

	/* the prelude on line 3 carries no case id */
	TD_CHECK_EQ(cf.count, 2);
	TD_CHECK_EQ(cf.line[0], 2);
	TD_CHECK_EQ(cf.line[1], 4);

	TD_CHECK(strcmp(td_casefile_comment(&cf, 2), "v_pline two points") == 0);
	/* everything from column five on, tabs included */
	TD_CHECK(strcmp(td_casefile_comment(&cf, 4),
	                "vqt_attributes\ttabs\tkept") == 0);
	TD_CHECK(strcmp(td_casefile_comment(&cf, 3), "") == 0);   /* prelude */
}

static void check_aes(const char *path)
{
	struct td_casefile cf;
	const char *why = NULL;

	printf("casefile: the AES form takes the comment from column seven\n");

	write_file(path, aes_text);
	TD_CHECK_EQ(td_casefile_read(&cf, path, TD_AREA_AES, &why), 0);

	TD_CHECK_EQ(cf.count, 2);
	TD_CHECK_EQ(cf.line[0], 2);
	TD_CHECK_EQ(cf.line[1], 4);

	/* the count columns and the value column must NOT end up in the
	 * comment -- that is what the five column rule would have done */
	TD_CHECK(strcmp(td_casefile_comment(&cf, 2), "wind_calc WC_WORK") == 0);
	TD_CHECK(strcmp(td_casefile_comment(&cf, 4),
	                "wind_get 0 WF_WXYWH\ttabs\tkept") == 0);
}

/*
 * An AES table that declares "!columns 8" carries an addr_in column between
 * the values and the comment (core/cases.h).  The reader has to take the
 * comment from column EIGHT there -- and it can, because unlike the area the
 * column count IS in the file.  A reader that assumed seven would name every
 * case of such a table after its addr_in column.
 */
static const char *aes8_text =
	"!columns\t8\n"
	"# an AES table with the addr_in column\n"
	"13\t0\t1\t1\t0\t-\ta0=0\tappl_find through a buffer\n"
	"+77\t0\t5\t0\t0\t-\t-\tgraf_handle, a prelude\n"
	"77\t0\t5\t0\t0\t-\t-\tgraf_handle\ttabs\tkept\n";

static void check_aes_eight_columns(const char *path)
{
	struct td_casefile cf;
	const char *why = NULL;

	printf("casefile: an AES table that declares eight columns takes the "
	       "comment from column eight\n");

	write_file(path, aes8_text);
	TD_CHECK_EQ(td_casefile_read(&cf, path, TD_AREA_AES, &why), 0);

	TD_CHECK_EQ(cf.count, 2);
	TD_CHECK_EQ(cf.line[0], 3);
	TD_CHECK_EQ(cf.line[1], 5);
	TD_CHECK(strcmp(td_casefile_comment(&cf, 3),
	                "appl_find through a buffer") == 0);
	TD_CHECK(strcmp(td_casefile_comment(&cf, 5),
	                "graf_handle\ttabs\tkept") == 0);
}

static void check_crc_is_area_independent(const char *vpath, const char *apath)
{
	struct td_casefile a, b;
	const char *why = NULL;

	printf("casefile: the CRC and the case count do not depend on the area\n");

	/* Reading the SAME file as either area must give the same identity: the
	 * CRC is over the raw bytes and the case ids come from the shared line
	 * iterator.  Only the comment differs. */
	write_file(vpath, aes_text);
	TD_CHECK_EQ(td_casefile_read(&a, vpath, TD_AREA_AES, &why), 0);
	TD_CHECK_EQ(td_casefile_read(&b, vpath, TD_AREA_VDI, &why), 0);
	TD_CHECK_EQ(a.crc, b.crc);
	TD_CHECK_EQ(a.count, b.count);
	TD_CHECK_EQ(a.line[0], b.line[0]);
	TD_CHECK_EQ(a.line[1], b.line[1]);
	/* ... and the VDI rule really would have got the comment wrong */
	TD_CHECK(strcmp(td_casefile_comment(&b, 2), "wind_calc WC_WORK") != 0);

	(void)apath;
}

int main(int argc, char **argv)
{
	const char *dir = (argc > 1) ? argv[1] : ".";
	char vpath[512], apath[512];

	snprintf(vpath, sizeof vpath, "%s/casefile-vdi.tsv", dir);
	snprintf(apath, sizeof apath, "%s/casefile-aes.tsv", dir);

	check_vdi(vpath);
	check_aes(apath);
	check_aes_eight_columns(apath);
	check_crc_is_area_independent(vpath, apath);

	return TD_TEST_RESULT();
}
