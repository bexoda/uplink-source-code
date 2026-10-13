
#include <sstream>

#include "soundgarden.h"

#include "app/app.h"
#include "app/globals.h"
#include "app/serialise.h"
#include "app/opengl.h"
#include "app/opengl_interface.h"

#include "mainmenu/mainmenu.h"

#include "game/game.h"
#include "game/data/data.h"

#include "world/world.h"
#include "world/player.h"
#include "world/message.h"
#include "world/generator/consequencegenerator.h"
#include "world/scheduler/seizegatewayevent.h"

#include "options/options.h"




SeizeGatewayEvent::SeizeGatewayEvent ()
{

	reason = NULL;
	gateway_id = 0;

}

SeizeGatewayEvent::~SeizeGatewayEvent ()
{


}

void SeizeGatewayEvent::SetName ( const char *newname )
{

	UplinkAssert ( strlen(newname) < SIZE_PERSON_NAME );
	name = newname;

}

void SeizeGatewayEvent::SetReason ( const char *newreason )
{

	reason = newreason;

}

void SeizeGatewayEvent::SetGatewayID ( int newid )
{

	gateway_id = newid;

}

void SeizeGatewayEvent::Run ()
{

	Person *person = game->GetWorld ()->GetPerson ( name.c_str () );
	UplinkAssert (person);

	ConsequenceGenerator::SeizeGateway ( person, reason.c_str () );

	//
	// If this is the player, then it's GAME OVER
	//

	if ( strcmp ( name.c_str (), "PLAYER" ) == 0 ) {

		if ( gateway_id == game->GetWorld ()->GetPlayer ()->gateway.id ) {

			std::ostringstream deathmsg;

			deathmsg << "Disavowed by Uplink Corporation at " << game->GetWorld ()->date.GetLongString ()
					 << "\nFor " << reason.c_str ()
					 << "\n";

			game->GameOver ( deathmsg.str ().c_str () );


		}
		else {

			// The player's old gateway was seized
			// Not quite as severe - not game over

			Message *m = new Message ();
			m->SetTo ( "PLAYER" );
			m->SetFrom ( "Uplink Corporation" );
			m->SetSubject ( "Gateway seizure" );
			m->SetBody ( "Your old gateway was recently seized by federal agents, shortly after you blew it up.\n"
						 "It seems that no evidence remains of the crime that you didn't commit.\n\n"
						 "A very lucky escape, if you ask us.\n" );
			m->Send ();

			// Uplink Corporation won't like you for this (it's reckless after all)
			// Other hackers love this - everybody knows you did it but
			// you got away with it scott free!

			game->GetWorld ()->GetPlayer ()->rating.ChangeNeuromancerScore ( NEUROMANCERCHANGE_GATEWAYNEARMISS );
			game->GetWorld ()->GetPlayer ()->rating.ChangeUplinkScore ( UPLINKCHANGE_GATEWAYNEARMISS );

		}

	}

}

void SeizeGatewayEvent::RunWarning ()
{

	//
	// Agents are near (2 of em!)
	//

	game->GetWorld ()->GetPlayer ()->gateway.IncrementProximity ( 2 );

}

char *SeizeGatewayEvent::GetShortString ()
{

	size_t shortstringsize = strlen(name.c_str ()) + 32;
	char *shortstring = new char [shortstringsize];
	UplinkSnprintf ( shortstring, shortstringsize, "Seize Gateway of %s", name.c_str () );
	return shortstring;

}

char *SeizeGatewayEvent::GetLongString ()
{

	std::ostringstream longstring;
	longstring << "Seize Gateway Event\n"
			   << "For : " << name << "\n"
			   << "GatewayID : " << gateway_id << "\n"
			   << "Reason : " << reason.c_str ();
	return UplinkStrdup ( longstring.str ().c_str () );

}


bool SeizeGatewayEvent::Load  ( FILE *file )
{

	LoadID ( file );

	if ( !UplinkEvent::Load ( file ) ) return false;

	if ( strcmp( game->GetLoadedSavefileVer(), "SAV59" ) >= 0 ) {
		if ( !LoadDynamicStringStatic ( name, SIZE_PERSON_NAME, file ) ) return false;
	}
	else {
		if ( !LoadFixedString ( name, SIZE_PERSON_NAME, file ) ) return false;
	}
	if ( !LoadDynamicStringPtr ( &reason, file ) ) return false;
	if ( !FileReadData ( &gateway_id, sizeof(gateway_id), 1, file ) ) return false;

	LoadID_END ( file );

	return true;

}

void SeizeGatewayEvent::Save  ( FILE *file )
{

	SaveID ( file );

	UplinkEvent::Save ( file );

	SaveDynamicString ( name, SIZE_PERSON_NAME, file );
	SaveDynamicString ( reason, file );
	fwrite ( &gateway_id, sizeof(gateway_id), 1, file );

	SaveID_END ( file );

}

void SeizeGatewayEvent::Print ()
{

	printf ( "Seize Gateway Event for %s, Gateway no. %d:\n", name.c_str (), gateway_id );
	UplinkEvent::Print ();
	printf ( "%s\n", reason.c_str () );


}

const char *SeizeGatewayEvent::GetID ()
{

	return "EVT_SEZ";

}

int   SeizeGatewayEvent::GetOBJECTID ()
{

	return OID_SEIZEGATEWAYEVENT;

}

