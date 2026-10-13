// Screenshot regression tests. Each scene drives the real game to a screen and compares the
// frame with uplink/tests/screens/<scene>.png, pixel for pixel. References are specific to the
// renderer they were made with (Mesa llvmpipe under Xvfb, as in CI); regenerate them with
//   UPLINK_UPDATE_SCREENS=1 xvfb-run -s "-screen 0 1024x768x24" ./uplink_screen_tests
// and look at every changed image before committing it.

#include <catch2/catch_test_macros.hpp>

#include "harness.h"
#include "screens.h"

#include "app/app.h"
#include "app/globals.h"
#include "options/options.h"
#include "mainmenu/mainmenu.h"
#include "game/game.h"
#include "interface/interface.h"
#include "interface/localinterface/localinterface.h"
#include "interface/localinterface/worldmap/worldmap_interface.h"

static void Check ( const char *scene )
{
	screens::Frame ( 3 );
	auto problem = screens::CompareWithReference ( scene, screens::Capture () );
	INFO ( problem );
	CHECK ( problem.empty () );
}

#define REQUIRE_DISPLAY() \
	if ( !screens::HaveDisplay () ) SKIP ( "no display; run under xvfb-run" ); \
	screens::BootGraphics ();

// A new agent's first connection: the Uplink Public Access Server.
TEST_CASE ( "screen: public access server welcome", "[screens]" )
{
	REQUIRE_DISPLAY ();
	app->GetOptions ()->SetOptionValue ( "game_firsttime", 1 );
	harness::NewWorld ( 1 );
	Check ( "welcome" );
}

// The gateway: HUD, clock, small world map, toolbar, then each of its panels.
TEST_CASE ( "screen: gateway panels", "[screens]" )
{
	REQUIRE_DISPLAY ();
	app->GetOptions ()->SetOptionValue ( "game_firsttime", 0 );
	harness::NewWorld ( 1 );
	Check ( "gateway" );

	struct { int code; const char *name; } panels [] = {
		{ SCREEN_FINANCE, "gateway_finance" },
		{ SCREEN_STATUS,  "gateway_status" },
		{ SCREEN_HW,      "gateway_hardware" },
		{ SCREEN_MEMORY,  "gateway_memory" },
	};
	for ( auto &p : panels ) {
		SECTION ( p.name ) {
			game->GetInterface ()->GetLocalInterface ()->RunScreen ( p.code );
			Check ( p.name );
		}
	}

	SECTION ( "gateway_map" ) {
		// What clicking the small map does (WorldMapInterface::FullScreenClick)
		game->GetInterface ()->GetLocalInterface ()->RunScreen ( SCREEN_NONE );
		WorldMapInterface::CreateWorldMapInterface ( WORLDMAP_LARGE );
		Check ( "gateway_map" );
	}
}
