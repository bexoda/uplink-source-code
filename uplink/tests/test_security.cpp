// Trace times and how a computer's defences react to being hacked.

#include <catch2/catch_test_macros.hpp>

#include <cstdio>
#include <cstring>
#include <string>

#include "harness.h"

#include "app/globals.h"
#include "game/game.h"
#include "game/data/data.h"
#include "world/world.h"
#include "world/player.h"
#include "world/computer/computer.h"
#include "world/computer/recordbank.h"

namespace {

Computer *FindComputer ( int type )
{
	DArray <Computer *> *comps = game->GetWorld ()->computers.ConvertToDArray ();
	Computer *found = nullptr;
	for ( int i = 0; i < comps->Size () && !found; ++i )
		if ( comps->ValidIndex (i) && comps->GetData (i)->TYPE == type &&
		     comps->GetData (i)->recordbank.GetRecordFromName ( RECORDBANK_ADMIN ) )
			found = comps->GetData (i);
	delete comps;
	return found;
}

// Give the player the admin account on comp, as a cracked password would
void GiveAdminCode ( Computer *comp )
{
	Record *admin = comp->recordbank.GetRecordFromName ( RECORDBANK_ADMIN );
	REQUIRE ( admin );
	char code [256];
	Computer::GenerateAccessCode ( RECORDBANK_ADMIN, admin->GetField ( RECORDBANK_PASSWORD ), code, sizeof code );
	game->GetWorld ()->GetPlayer ()->GiveCode ( comp->ip.c_str (), code );
}

}

TEST_CASE ( "routing through a central mainframe you have an account on slows the trace by 1.3x", "[security][trace]" )
{
	harness::NewWorld ( 3 );
	Computer *mainframe = FindComputer ( COMPUTER_TYPE_CENTRALMAINFRAME );
	REQUIRE ( mainframe );
	GiveAdminCode ( mainframe );
	REQUIRE ( game->GetWorld ()->GetPlayer ()->HasAccount ( mainframe->ip.c_str () ) == 1 );

	char router [SIZE_VLOCATION_IP];
	snprintf ( router, sizeof router, "%s", mainframe->ip.c_str () );
	int expected = (int) ( (int) ( mainframe->tracespeed * TRACESPEED_MODIFIER_ADMINACCESS ) * TRACESPEED_MODIFIER_CENTRALMAINFRAME );
	CHECK ( game->GetWorld ()->GetPlayer ()->TimeToTrace ( mainframe->ip.c_str (), router ) == expected );
}
