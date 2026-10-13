// Hacking tools, run the way the player runs them: launched from the task manager, clicked
// onto a remote screen's button, and ticked until done, against a computer from a seeded world.
// These guard the tools against refactoring (a const change once disabled every SetTarget).

#include <catch2/catch_test_macros.hpp>

#include "harness.h"

#include "app/app.h"
#include "app/globals.h"
#include "app/serialise.h"
#include "options/options.h"
#include "game/game.h"
#include "interface/interface.h"
#include "interface/remoteinterface/remoteinterface.h"
#include "interface/taskmanager/taskmanager.h"
#include "world/world.h"
#include "world/player.h"
#include "world/connection.h"
#include "world/computer/computer.h"
#include "world/computer/databank.h"
#include "world/computer/logbank.h"
#include "world/computer/computerscreen/genericscreen.h"
#include "world/computer/computerscreen/passwordscreen.h"
#include "world/computer/databank.h"
#include "eclipse.h"
#include <cstring>
#include <string>
#include "vanbakel.h"

namespace {

double clockms = 5000000.0;

// Advance the UI clock and run the task manager, which ticks every task.
void Step ( int frames )
{
	for ( int i = 0; i < frames; ++i ) {
		clockms += 100.0;
		EclSetFixedTime ( clockms );
		game->GetInterface ()->GetTaskManager ()->Update ();
		EclUpdateAllAnimations ();
	}
}

int FindScreen ( Computer *comp, int objectid, int generictype = -1 )
{
	for ( int i = 0; i < comp->screens.Size (); ++i ) {
		if ( !comp->screens.ValidIndex (i) ) continue;
		ComputerScreen *cs = comp->screens.GetData (i);
		if ( cs->GetOBJECTID () != objectid ) continue;
		if ( generictype != -1 && ((GenericScreen *) cs)->SCREEN_TYPE != generictype ) continue;
		return i;
	}
	return -1;
}

int CountLogs ( LogBank &lb )
{
	int n = 0;
	for ( int i = 0; i < lb.logs.Size (); ++i )
		if ( lb.logs.ValidIndex (i) && lb.logs.GetData (i) ) ++n;
	return n;
}

// A computer with a password screen, a file server holding files, and logs.
Computer *FindTarget ()
{
	DArray <Computer *> *comps = game->GetWorld ()->computers.ConvertToDArray ();
	Computer *found = nullptr;
	for ( int i = 0; i < comps->Size () && !found; ++i ) {
		if ( !comps->ValidIndex (i) ) continue;
		Computer *c = comps->GetData (i);
		if ( FindScreen ( c, OID_PASSWORDSCREEN ) != -1 &&
		     FindScreen ( c, OID_GENERICSCREEN, SCREEN_FILESERVERSCREEN ) != -1 &&
		     FindScreen ( c, OID_LOGSCREEN ) != -1 &&
		     c->databank.NumDataFiles () > 0 && CountLogs ( c->logbank ) > 0 )
			found = c;
	}
	delete comps;
	return found;
}

// Connect the player directly to comp and show one of its screens.
void Show ( Computer *comp, int screenindex )
{
	Connection *conn = game->GetWorld ()->GetPlayer ()->GetConnection ();
	conn->Disconnect ();
	conn->Reset ();
	conn->AddVLocation ( comp->ip.c_str () );
	conn->Connect ();
	game->GetInterface ()->GetRemoteInterface ()->RunNewLocation ();
	game->GetInterface ()->GetRemoteInterface ()->RunScreen ( screenindex, comp );
	Step ( 2 );
}

// Launch a tool and click it onto a button of the current screen.
void RunOn ( const char *tool, float version, UplinkObject *target, const char *button, int index )
{
	TaskManager *tm = game->GetInterface ()->GetTaskManager ();
	tm->RunSoftware ( tool, version );
	Step ( 1 );
	REQUIRE ( EclGetButton ( button ) != nullptr );
	tm->SetProgramTarget ( target, button, index );
}

Computer *Setup ()
{
	harness::Boot ();
	app->GetOptions ()->SetOptionValue ( "game_firsttime", 0 );
	harness::NewWorld ( 7 );
	game->GetWorld ()->date.DeActivate ();
	if ( !EclGetButton ( "svb_scalebar" ) ) SvbCreateInterface ( 335, 15 );
	Computer *comp = FindTarget ();
	REQUIRE ( comp != nullptr );
	return comp;
}

}

TEST_CASE ( "Log_Deleter removes the log it is clicked on", "[tools]" )
{
	Computer *comp = Setup ();
	Show ( comp, FindScreen ( comp, OID_LOGSCREEN ) );

	int before = CountLogs ( comp->logbank );
	int index = -1;
	for ( int i = 0; i < comp->logbank.logs.Size () && index == -1; ++i )
		if ( comp->logbank.logs.ValidIndex (i) && comp->logbank.logs.GetData (i) ) index = i;
	REQUIRE ( index != -1 );

	char button [64];
	snprintf ( button, sizeof button, "logscreen_log %d", index );
	RunOn ( "Log_Deleter", 1.0f, &comp->logbank, button, index );
	Step ( 600 );

	// Version 1.0 leaves a "deleted" marker in the log's place
	CHECK ( CountLogs ( comp->logbank ) == before );
	REQUIRE ( comp->logbank.logs.ValidIndex ( index ) );
	CHECK ( comp->logbank.logs.GetData ( index )->TYPE == LOG_TYPE_DELETED );
}

namespace {

int FirstFile ( DataBank &db, bool encrypted = false )
{
	for ( int i = 0; i < db.NumDataFiles (); ++i ) {
		Data *d = db.GetDataFile (i);
		if ( d && ( !encrypted || d->encrypted > 0 ) ) return db.GetMemoryIndex (i);
	}
	return -1;
}

bool PlayerHasCodeFor ( const char *ip )
{
	return game->GetWorld ()->GetPlayer ()->codes.LookupTree ( (char *) ip ) != nullptr;
}

}

TEST_CASE ( "File_Copier copies a file into the gateway's memory", "[tools]" )
{
	Computer *comp = Setup ();
	Show ( comp, FindScreen ( comp, OID_GENERICSCREEN, SCREEN_FILESERVERSCREEN ) );

	int memoryindex = FirstFile ( comp->databank );
	REQUIRE ( memoryindex != -1 );
	std::string title = comp->databank.GetData ( memoryindex )->title;
	DataBank &gateway = game->GetWorld ()->GetPlayer ()->gateway.databank;
	REQUIRE_FALSE ( gateway.ContainsData ( title.c_str () ) );

	RunOn ( "File_Copier", 1.0f, &comp->databank, "fileserverscreen_file 0", memoryindex );
	Step ( 600 );											// download
	game->GetInterface ()->GetTaskManager ()->SetProgramTarget ( &gateway, "memory_block 0", -1 );
	Step ( 20 );

	CHECK ( gateway.ContainsData ( title.c_str () ) );
	CHECK ( comp->databank.GetData ( memoryindex ) != nullptr );	// still on the server
}

TEST_CASE ( "File_Deleter deletes a file from the server", "[tools]" )
{
	Computer *comp = Setup ();
	Show ( comp, FindScreen ( comp, OID_GENERICSCREEN, SCREEN_FILESERVERSCREEN ) );

	int memoryindex = FirstFile ( comp->databank );
	REQUIRE ( memoryindex != -1 );
	int files = comp->databank.NumDataFiles ();

	RunOn ( "File_Deleter", 1.0f, &comp->databank, "fileserverscreen_file 0", memoryindex );
	Step ( 600 );

	CHECK ( comp->databank.NumDataFiles () == files - 1 );
	CHECK ( comp->databank.GetData ( memoryindex ) == nullptr );
}

TEST_CASE ( "Decrypter removes a file's encryption", "[tools]" )
{
	harness::Boot ();
	app->GetOptions ()->SetOptionValue ( "game_firsttime", 0 );
	harness::NewWorld ( 7 );
	game->GetWorld ()->date.DeActivate ();
	if ( !EclGetButton ( "svb_scalebar" ) ) SvbCreateInterface ( 335, 15 );

	// any file server holding an encrypted file
	DArray <Computer *> *comps = game->GetWorld ()->computers.ConvertToDArray ();
	Computer *comp = nullptr;
	for ( int i = 0; i < comps->Size () && !comp; ++i )
		if ( comps->ValidIndex (i) && FindScreen ( comps->GetData (i), OID_GENERICSCREEN, SCREEN_FILESERVERSCREEN ) != -1 &&
		     FirstFile ( comps->GetData (i)->databank, true ) != -1 )
			comp = comps->GetData (i);
	delete comps;
	REQUIRE ( comp != nullptr );

	Show ( comp, FindScreen ( comp, OID_GENERICSCREEN, SCREEN_FILESERVERSCREEN ) );
	int memoryindex = FirstFile ( comp->databank, true );
	Data *data = comp->databank.GetData ( memoryindex );
	REQUIRE ( data->encrypted > 0 );

	RunOn ( "Decrypter", 7.0f, &comp->databank, "fileserverscreen_file 0", memoryindex );
	Step ( 3000 );

	CHECK ( data->encrypted == 0 );
}

TEST_CASE ( "Password_Breaker breaks a password and gives the player the code", "[tools]" )
{
	Computer *comp = Setup ();
	int ps = FindScreen ( comp, OID_PASSWORDSCREEN );
	Show ( comp, ps );
	REQUIRE_FALSE ( PlayerHasCodeFor ( comp->ip.c_str () ) );

	RunOn ( "Password_Breaker", 1.0f, comp->screens.GetData ( ps ), "passwordscreen_password", -1 );
	Step ( 6000 );

	CHECK ( PlayerHasCodeFor ( comp->ip.c_str () ) );
	CHECK ( strcmp ( EclGetButton ( "passwordscreen_password" )->caption,
	                 ((PasswordScreen *) comp->screens.GetData ( ps ))->password.c_str () ) == 0 );
}

TEST_CASE ( "Log_UnDeleter restores a deleted log", "[tools]" )
{
	Computer *comp = Setup ();
	Show ( comp, FindScreen ( comp, OID_LOGSCREEN ) );

	int index = -1;
	for ( int i = 0; i < comp->logbank.logs.Size () && index == -1; ++i )
		if ( comp->logbank.logs.ValidIndex (i) && comp->logbank.internallogs.ValidIndex (i) ) index = i;
	REQUIRE ( index != -1 );
	int originaltype = comp->logbank.logs.GetData ( index )->TYPE;
	REQUIRE ( originaltype != LOG_TYPE_DELETED );

	char button [64];
	snprintf ( button, sizeof button, "logscreen_log %d", index );
	RunOn ( "Log_Deleter", 1.0f, &comp->logbank, button, index );
	Step ( 600 );
	REQUIRE ( comp->logbank.logs.GetData ( index )->TYPE == LOG_TYPE_DELETED );

	RunOn ( "Log_UnDeleter", 1.0f, &comp->logbank, button, index );
	Step ( 600 );
	CHECK ( comp->logbank.logs.GetData ( index )->TYPE == originaltype );
}
