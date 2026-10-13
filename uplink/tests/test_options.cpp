// The options file: a fixed-layout binary file the game writes to the user's directory. Its
// format must not change, so a file written with the default options is kept as a fixture.
// If the format is changed on purpose, regenerate with:  UPLINK_UPDATE_OPTIONS=1 ./uplink_tests "[options]"

#include <catch2/catch_test_macros.hpp>

#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

#include "harness.h"

#include "app/app.h"
#include "app/globals.h"
#include "options/options.h"

#ifndef UPLINK_TEST_FIXTURES
#error UPLINK_TEST_FIXTURES must point at uplink/tests/fixtures
#endif

static const std::string kFixture = std::string ( UPLINK_TEST_FIXTURES ) + "/options_default.bin";

static std::vector<unsigned char> ReadFile ( const std::string &path )
{
	std::vector<unsigned char> out;
	FILE *f = fopen ( path.c_str (), "rb" );
	if ( !f ) return out;
	unsigned char buf [65536];
	size_t n;
	while ( ( n = fread ( buf, 1, sizeof buf, f ) ) > 0 )
		out.insert ( out.end (), buf, buf + n );
	fclose ( f );
	return out;
}

static void WriteFile ( const std::string &path, const std::vector<unsigned char> &bytes )
{
	FILE *f = fopen ( path.c_str (), "wb" );
	REQUIRE ( f );
	fwrite ( bytes.data (), 1, bytes.size (), f );
	fclose ( f );
}

// Options::Save always writes <userpath>options
static std::vector<unsigned char> SaveOptions ( Options &options )
{
	options.Save ( NULL );
	return ReadFile ( std::string ( app->userpath ) + "options" );
}

TEST_CASE ( "the options file keeps its format and round-trips", "[options]" )
{
	harness::Boot ();

	Options options;
	options.CreateDefaultOptions ();
	options.SetOptionValue ( "test_option", 42, "A tooltip that is saved too", true, false );
	options.SetThemeName ( "graphics" );

	std::vector<unsigned char> saved = SaveOptions ( options );
	REQUIRE ( saved.size () > 0 );

	if ( getenv ( "UPLINK_UPDATE_OPTIONS" ) )
		WriteFile ( kFixture, saved );

	std::vector<unsigned char> fixture = ReadFile ( kFixture );
	REQUIRE ( fixture.size () == saved.size () );
	CHECK ( fixture == saved );

	Options loaded;
	REQUIRE ( loaded.Load ( NULL ) );
	CHECK ( loaded.GetOptionValue ( "test_option" ) == 42 );
	CHECK ( std::string ( loaded.GetOption ( "test_option" )->tooltip ) == "A tooltip that is saved too" );
	CHECK ( std::string ( loaded.GetThemeName () ) == "graphics" );
	CHECK ( SaveOptions ( loaded ) == saved );
}
