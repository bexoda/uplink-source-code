// Golden save: a SAV62 file produced by the game before modernisation started (seed 1, one game
// day after the start). Refactors must keep loading it and writing it back unchanged. If the save
// format is changed on purpose, bump SAVEFILE_VERSION, keep a loader for the old version, and
// regenerate with:  ./uplink_tests "[.regenerate]"

#include <catch2/catch_test_macros.hpp>
#include <catch2/catch_message.hpp>

#include <string>
#include <vector>
#include <zlib.h>

#include "harness.h"

#ifndef UPLINK_TEST_FIXTURES
#error UPLINK_TEST_FIXTURES must point at uplink/tests/fixtures
#endif

static const std::string kGolden = std::string ( UPLINK_TEST_FIXTURES ) + "/sav62_seed1_day1.sav.gz";

static void MakeGoldenWorld ()
{
	harness::NewWorld ( 1 );
	harness::RunWorld ( 24 * 60 * 60 );
}

static std::vector<unsigned char> ReadGz ( const std::string &path )
{
	std::vector<unsigned char> out;
	gzFile f = gzopen ( path.c_str (), "rb" );
	if ( !f ) return out;
	unsigned char buf [65536];
	int n;
	while ( ( n = gzread ( f, buf, sizeof buf ) ) > 0 )
		out.insert ( out.end (), buf, buf + n );
	gzclose ( f );
	return out;
}

TEST_CASE ( "the golden SAV62 save loads and writes back unchanged", "[save][golden]" )
{
	auto golden = ReadGz ( kGolden );
	REQUIRE ( golden.size () > 1000 );

	harness::Boot ();
	REQUIRE ( harness::LoadBytes ( golden ) );
	auto resaved = harness::SaveBytes ();
	REQUIRE ( resaved.size () == golden.size () );
	CHECK ( resaved == golden );
}

TEST_CASE ( "world generation and simulation still produce the golden save", "[world][golden]" )
{
#ifndef __GLIBC__
	SKIP ( "the golden world was generated with glibc's rand()" );
#endif
	auto golden = ReadGz ( kGolden );
	REQUIRE ( golden.size () > 1000 );

	MakeGoldenWorld ();
	auto now = harness::SaveBytes ();
	REQUIRE ( now.size () == golden.size () );
	CHECK ( now == golden );
}

TEST_CASE ( "regenerate the golden save", "[.regenerate]" )
{
	MakeGoldenWorld ();
	auto bytes = harness::SaveBytes ();
	gzFile f = gzopen ( kGolden.c_str (), "wb9" );
	REQUIRE ( f );
	REQUIRE ( gzwrite ( f, bytes.data (), (unsigned) bytes.size () ) == (int) bytes.size () );
	gzclose ( f );
	UNSCOPED_INFO ( "wrote " << kGolden );
}
