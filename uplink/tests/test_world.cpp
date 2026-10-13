#include <catch2/catch_test_macros.hpp>

#include "harness.h"

#include "app/globals.h"
#include "game/game.h"
#include "world/world.h"
#include "world/player.h"
#include "world/date.h"

TEST_CASE ( "a new world is generated", "[world]" )
{
	harness::NewWorld ( 1 );
	REQUIRE ( game->GetWorld () != nullptr );
	REQUIRE ( game->GetWorld ()->GetPlayer () != nullptr );
}

TEST_CASE ( "save, load, save gives identical bytes", "[save]" )
{
	harness::NewWorld ( 2 );
	auto first = harness::SaveBytes ();
	REQUIRE ( first.size () > 1000 );

	REQUIRE ( harness::LoadBytes ( first ) );
	auto second = harness::SaveBytes ();

	REQUIRE ( first.size () == second.size () );
	CHECK ( first == second );
}

TEST_CASE ( "the same seed generates the same world", "[world][determinism]" )
{
	harness::NewWorld ( 3 );
	auto a = harness::SaveBytes ();
	harness::NewWorld ( 3 );
	auto b = harness::SaveBytes ();
	REQUIRE ( a.size () == b.size () );
	CHECK ( a == b );

	harness::NewWorld ( 30 );
	CHECK ( harness::SaveBytes () != a );
}

TEST_CASE ( "a world that has run for a while still round-trips", "[save][sim]" )
{
	harness::NewWorld ( 4 );
	auto fresh = harness::SaveBytes ();
	Date start;
	start.SetDate ( &game->GetWorld ()->date );

	harness::RunWorld ( 2 * 24 * 60 * 60 );

	Date expected;
	expected.SetDate ( &start );
	expected.AdvanceDay ( 2 );
	CHECK ( game->GetWorld ()->date.After ( &expected ) == false );
	CHECK ( game->GetWorld ()->date.Before ( &expected ) == false );

	auto first = harness::SaveBytes ();
	CHECK ( first != fresh );
	REQUIRE ( harness::LoadBytes ( first ) );
	auto second = harness::SaveBytes ();
	REQUIRE ( first.size () == second.size () );
	CHECK ( first == second );
}

