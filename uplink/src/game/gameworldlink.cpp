#include <cstring>

#include "eclipse.h"
#include "redshirt.h"
#include "soundgarden.h"

#include "app/globals.h"
#include "app/miscutils.h"
#include "app/opengl_interface.h"

#include "game/game.h"
#include "game/data/data.h"
#include "game/gameworldlink.h"

#include "interface/interface.h"
#include "interface/remoteinterface/remoteinterface.h"
#include "interface/remoteinterface/linksscreen_interface.h"
#include "interface/remoteinterface/dialogscreen_interface.h"
#include "interface/taskmanager/taskmanager.h"

#include "world/world.h"
#include "world/player.h"
#include "world/computer/computerscreen/computerscreen.h"
#include "world/computer/computerscreen/dialogscreen.h"

double GameWorldLink::TimeMs ()
{
	return EclGetAccurateTime ();
}

void GameWorldLink::ShowDisconnected ()
{
	game->GetInterface ()->GetRemoteInterface ()->RunNewLocation ();
	game->GetInterface ()->GetRemoteInterface ()->RunScreen ( 2 );
}

void GameWorldLink::GameSpeedChanged ()
{
	EclDirtyButton ( "hud_speed 0" );
	EclDirtyButton ( "hud_speed 1" );
	EclDirtyButton ( "hud_speed 2" );
	EclDirtyButton ( "hud_speed 3" );
}

void GameWorldLink::PlaySound ( const char *filename, bool synchronised )
{
	SgPlaySound ( RsArchiveFileOpen ( filename ), filename, synchronised );
}

void GameWorldLink::CriticalError ( const char *message )
{
	create_msgbox ( "Critical Error", message );
}

void GameWorldLink::SetLanAlarm ( bool on )
{
	if ( !on && EclIsSuperHighlighted ( "hud_lanview" ) ) {
		EclSuperUnHighlight ( "hud_lanview" );
	}
	else if ( on && !EclIsSuperHighlighted ( "hud_lanview" ) ) {
		EclSuperHighlight ( "hud_lanview" );
		SgPlaySound ( RsArchiveFileOpen ( "sounds/siren.wav" ), "sounds/sirens.wav" );
	}
}

void GameWorldLink::SetRemoteSecurity ( const char *name, int level )
{
	game->GetInterface ()->GetRemoteInterface ()->SetSecurity ( name, level );
}

void GameWorldLink::PlayerLinksChanged ( LList <char *> *links )
{
	// Only when the player is at home looking at the links screen
	if ( strcmp ( game->GetWorld ()->GetPlayer ()->remotehost.c_str (), IP_LOCALHOST ) == 0 &&
	     game->GetInterface ()->GetRemoteInterface ()->currentscreenindex == 0 ) {
		LinksScreenInterface *lsi = (LinksScreenInterface *) game->GetInterface ()->GetRemoteInterface ()->GetInterfaceScreen ();
		lsi->SetFullList ( links );
		lsi->ApplyFilter ( NULL );
	}
}

void GameWorldLink::DialogWidgetRemoved ( DialogScreen *screen, DialogScreenWidget *widget )
{
	if ( game->GetInterface ()->GetRemoteInterface ()->GetComputerScreen () == screen )
		DialogScreenInterface::RemoveWidget ( widget, screen->GetComputer () );
}

void GameWorldLink::RunPlayerSoftware ( const char *name, float version )
{
	game->GetInterface ()->GetTaskManager ()->RunSoftware ( name, version );
}

Computer *GameWorldLink::ScreenComputer ()
{
	return game->GetInterface ()->GetRemoteInterface ()->GetComputerScreen ()->GetComputer ();
}

const char *GameWorldLink::ScreenSecurityName ()
{
	return game->GetInterface ()->GetRemoteInterface ()->security_name.c_str ();
}
