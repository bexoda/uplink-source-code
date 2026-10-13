// The software and hardware tables come from data/upgrades.tsv. Its rows were generated from
// the tables previously compiled into the game, and checked equal before those were removed.

#include <catch2/catch_test_macros.hpp>

#include <cstdio>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>

#include "harness.h"

#include "game/data/upgrades.h"

#ifndef UPLINK_DATA_DIR
#error UPLINK_DATA_DIR must point at uplink/data
#endif

static const std::string kFile = std::string ( UPLINK_DATA_DIR ) + "/upgrades.tsv";

static std::string Read ( const std::string &path )
{
	std::ifstream in ( path, std::ios::binary );
	std::stringstream ss;
	ss << in.rdbuf ();
	return ss.str ();
}

TEST_CASE ( "data/upgrades.tsv loads and writes back unchanged", "[upgrades]" )
{
	std::string error;
	REQUIRE ( LoadUpgrades ( kFile.c_str (), &error ) );
	CHECK ( SoftwareUpgrades ().size () == 73 );
	CHECK ( HardwareUpgrades ().size () == 19 );
	CHECK ( UpgradesToText () == Read ( kFile ) );

	const ComputerUpgrade *d = GetSoftwareUpgrade ( "Decrypter" );
	REQUIRE ( d );
	CHECK ( d->cost == 800 );
	CHECK ( d->data == 1 );
	CHECK ( std::string ( d->description ).rfind ( "DECRYPTER V1.0\nTYPE: software.Cracker\n\n", 0 ) == 0 );
	CHECK ( GetHardwareUpgrade ( "Gateway Self Destruct" )->cost == 20000 );
	CHECK ( GetSoftwareUpgrade ( "No such program" ) == NULL );
}

TEST_CASE ( "a broken upgrades file is refused with the line it fails on", "[upgrades]" )
{
	harness::Boot ();
	std::string path = harness::ScratchFile ( "bad_upgrades.tsv" );
	{
		std::ofstream out ( path );
		out << "# comment\n"
		    << "software\tDecrypter\t4\t800\t2\t1\tfine\n"
		    << "software\tDecrypter\t4\tlots\t2\t2\tcost is not a number\n";
	}
	std::string error;
	CHECK_FALSE ( LoadUpgrades ( path.c_str (), &error ) );
	CHECK ( error.find ( ":3:" ) != std::string::npos );
	CHECK ( SoftwareUpgrades ().size () == 73 );		// the tables in use are untouched
	remove ( path.c_str () );
}
