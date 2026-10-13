#include "harness.h"

#include <cstdio>
#include <cstdlib>
#include <stdexcept>

#include "uplink.h"
#include "app/app.h"
#include "app/globals.h"
#include "app/binreloc.h"
#include "app/serialise.h"
#include "app/miscutils.h"
#include "game/game.h"
#include "world/world.h"
#include "world/date.h"
#include "options/options.h"
#include "eclipse.h"
#include "vanbakel.h"

namespace harness {

static bool booted = false;

void Boot ()
{
	if ( booted ) return;
	booted = true;

	setenv ( "SDL_AUDIODRIVER", "dummy", 0 );	// tests never need sound

	char *exe = br_find_exe ( NULL );
	Init_App ( exe );
	free ( exe );

	char arg0 [] = "uplink_tests";
	char *argv [] = { arg0, nullptr };
	Init_Options ( 1, argv );

	if ( !Load_Data () )
		throw std::runtime_error ( "Load_Data failed: run from a directory holding the .dat files" );

	Init_Game ();
	Init_Graphics ();	// theme only
	Init_Fonts ();		// FreeType metrics: the UI lays out text while a world is generated
}

void NewWorld ( unsigned seed )
{
	Boot ();
	// Clear the previous game's buttons, as the game does whenever it leaves a game: the
	// interface only builds screens whose buttons are not registered already.
	EclReset ( app->GetOptions ()->GetOptionValue ( "graphics_screenwidth" ),
	           app->GetOptions ()->GetOptionValue ( "graphics_screenheight" ) );
	delete game;
	game = new Game ();
	srand ( seed );
	game->NewGame ();
}

void Shutdown ()
{
	if ( !booted ) return;
	EclReset ( app->GetOptions ()->GetOptionValue ( "graphics_screenwidth" ),
	           app->GetOptions ()->GetOptionValue ( "graphics_screenheight" ) );
	delete game;
	game = NULL;
	SvbReset ();		// as App::Close does: running tools outlive the game
}

std::string ScratchFile ( const char *name )
{
	return std::string ( app->usertmppath ) + name;
}

std::vector<unsigned char> SaveBytes ()
{
	std::string path = ScratchFile ( "harness.sav" );
	MakeDirectory ( app->usertmppath.c_str () );
	FILE *f = fopen ( path.c_str (), "wb" );
	if ( !f ) throw std::runtime_error ( "cannot write " + path );
	game->Save ( f );
	fclose ( f );

	f = fopen ( path.c_str (), "rb" );
	std::vector<unsigned char> bytes;
	unsigned char buf [65536];
	size_t n;
	while ( ( n = fread ( buf, 1, sizeof buf, f ) ) > 0 )
		bytes.insert ( bytes.end (), buf, buf + n );
	fclose ( f );
	return bytes;
}

bool LoadBytes ( const std::vector<unsigned char> &bytes )
{
	std::string path = ScratchFile ( "harness.sav" );
	FILE *f = fopen ( path.c_str (), "wb" );
	fwrite ( bytes.data (), 1, bytes.size (), f );
	fclose ( f );

	delete game;
	game = new Game ();
	f = fopen ( path.c_str (), "rb" );
	bool ok = game->LoadGame ( f );
	fclose ( f );
	return ok;
}

void RunWorld ( int gameSeconds )
{
	// World::Update advances the clock from wall time; step it explicitly instead so a run is
	// fast and repeatable. Two seconds is the world's own update interval.
	Date &date = game->GetWorld ()->date;
	date.DeActivate ();
	for ( int t = 0; t < gameSeconds; t += 2 ) {
		date.AdvanceSecond ( 2 );
		game->GetWorld ()->Update ();
	}
	date.Activate ();
}

}
