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

// ---------------------------------------------------------------- adaptive security

#include <fstream>

#include "game/data/securityalert.h"
#include "world/connection.h"
#include "world/computer/security.h"
#include "world/computer/securitysystem.h"

namespace {

// A fresh computer of the given type with a clean record of break-ins
Computer *Calm ( int type )
{
	harness::NewWorld ( 3 );
	Computer *comp = FindComputer ( type );
	REQUIRE ( comp );
	comp->AddToRecentHacks ( -comp->NumRecentHacks () );
	REQUIRE ( comp->Alert ().level == 0 );
	return comp;
}

}

TEST_CASE ( "alert level follows the break-ins a computer has noticed", "[security][adaptive]" )
{
	harness::Boot ();
	const char *expect [] = { "Calm", "Wary", "Wary", "Alarmed", "Alarmed", "Alarmed", "Lockdown", "Lockdown" };
	for ( int hacks = 0; hacks < 8; ++hacks ) {
		INFO ( hacks << " recent hacks" );
		CHECK ( SecurityAlertFor ( hacks ).name == expect [hacks] );
	}
	CHECK ( SecurityAlertFor ( 100 ).name == "Lockdown" );
}

TEST_CASE ( "break-ins fade: the alert drops as the weeks pass", "[security][adaptive]" )
{
	Computer *comp = Calm ( COMPUTER_TYPE_CENTRALMAINFRAME );
	Date &date = game->GetWorld ()->date;

	comp->AddToRecentHacks ( 4 );						// in one month
	CHECK ( comp->Alert ().name == "Alarmed" );

	date.SetDate ( 0, 0, 0, 1, 2, 2010 );				// a new month: last month's 4 start to fade
	comp->UpdateRecentHacks ();
	for ( int week = 0; week < 4; ++week ) {
		date.SetDate ( 0, 0, 0, 8 + 7 * week, 2, 2010 );
		comp->UpdateRecentHacks ();
	}
	CHECK ( comp->NumRecentHacks () == 0 );
	CHECK ( comp->Alert ().name == "Calm" );
}

TEST_CASE ( "an alarmed computer traces faster", "[security][adaptive][trace]" )
{
	Computer *comp = Calm ( COMPUTER_TYPE_CENTRALMAINFRAME );
	comp->SetTraceSpeed ( 100 );						// big enough to see the ratios
	GiveAdminCode ( comp );
	char router [SIZE_VLOCATION_IP];
	snprintf ( router, sizeof router, "%s", comp->ip.c_str () );

	int calm = Player::TimeToTrace ( comp->ip.c_str (), router );
	REQUIRE ( calm > 10 );
	comp->AddToRecentHacks ( 3 );
	CHECK ( Player::TimeToTrace ( comp->ip.c_str (), router ) == (int) ( calm * 0.70f ) );
	comp->AddToRecentHacks ( 3 );
	CHECK ( Player::TimeToTrace ( comp->ip.c_str (), router ) == (int) ( calm * 0.55f ) );
}

TEST_CASE ( "an alarmed computer's security systems count one level higher, up to 5", "[security][adaptive]" )
{
	Computer *comp = Calm ( COMPUTER_TYPE_CENTRALMAINFRAME );
	SecuritySystem three, five;
	three.SetLevel ( 3 );
	five.SetLevel ( 5 );

	CHECK ( comp->EffectiveSecurityLevel ( &three ) == 3 );
	comp->AddToRecentHacks ( 1 );						// wary: no change yet
	CHECK ( comp->EffectiveSecurityLevel ( &three ) == 3 );
	comp->AddToRecentHacks ( 2 );						// alarmed
	CHECK ( comp->EffectiveSecurityLevel ( &three ) == 4 );
	CHECK ( comp->EffectiveSecurityLevel ( &five ) == 5 );
	CHECK ( three.level == 3 );							// nothing is changed in the save
}

TEST_CASE ( "a computer in lockdown changes its passwords when the player connects", "[security][adaptive]" )
{
	Computer *comp = Calm ( COMPUTER_TYPE_INTERNALSERVICESMACHINE );
	auto password = [&] { return std::string ( comp->recordbank.GetRecordFromName ( RECORDBANK_ADMIN )->GetField ( RECORDBANK_PASSWORD ) ); };
	Connection *conn = game->GetWorld ()->GetPlayer ()->GetConnection ();

	std::string before = password ();
	conn->Reset ();
	conn->AddVLocation ( comp->ip.c_str () );
	conn->Connect ();
	CHECK ( password () == before );					// calm: nothing happens
	conn->Disconnect ();

	comp->AddToRecentHacks ( 6 );
	conn->Reset ();
	conn->AddVLocation ( comp->ip.c_str () );
	conn->Connect ();
	CHECK ( password () != before );
	conn->Disconnect ();
	conn->Reset ();
}

TEST_CASE ( "a broken security alert table is refused", "[security][adaptive]" )
{
	harness::Boot ();
	std::string path = harness::ScratchFile ( "bad_security.tsv" );
	std::ofstream ( path ) << "0\tCalm\t0\t1.0\t0\t0\n1\tWary\t0\t0.9\t0\t0\n";		// thresholds must rise
	std::string error;
	CHECK_FALSE ( LoadSecurityAlerts ( path.c_str (), &error ) );
	CHECK ( error.find ( ":2:" ) != std::string::npos );
	CHECK ( SecurityAlerts ().size () == 4 );
	remove ( path.c_str () );
}

#include "eclipse.h"
#include "vanbakel.h"
#include "interface/taskmanager/securitybypass.h"

TEST_CASE ( "a version 3 proxy bypass beats a calm level 3 proxy, not an alarmed one", "[security][adaptive][tools]" )
{
	Computer *comp = Calm ( COMPUTER_TYPE_CENTRALMAINFRAME );
	comp->security.AddSystem ( SECURITY_TYPE_PROXY, 3 );
	int index = -1;
	for ( int i = 0; i < comp->security.NumSystems (); ++i ) {
		SecuritySystem *ss = comp->security.GetSystem (i);
		if ( ss && ss->TYPE == SECURITY_TYPE_PROXY && ss->level == 3 ) index = i;
	}
	REQUIRE ( index != -1 );

	Connection *conn = game->GetWorld ()->GetPlayer ()->GetConnection ();
	conn->Reset ();
	conn->AddVLocation ( comp->ip.c_str () );
	conn->Connect ();

	EclRegisterButton ( 0, 0, 10, 10, "", "", "test_proxy" );

	auto tryBypass = [&] () {
		SecurityBypass *sb = new SecurityBypass ( SECURITY_TYPE_PROXY );
		sb->SetVersion ( 3.0f );
		SvbRegisterTask ( "Proxy_Bypass", sb );
		sb->SetTarget ( &comp->security, "test_proxy", index );
		int status = sb->status;
		SvbRemoveTask ( sb );						// ends the bypass and frees the task
		return status;
	};

	CHECK ( tryBypass () == 1 );						// calm: level 3, version 3 works
	comp->AddToRecentHacks ( 3 );					// alarmed: counts as level 4
	CHECK ( tryBypass () == 0 );

	EclRemoveButton ( "test_proxy" );
	conn->Disconnect ();
	conn->Reset ();
}
