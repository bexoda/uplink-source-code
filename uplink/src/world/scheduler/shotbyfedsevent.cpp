
#include <sstream>


#include "app/app.h"
#include "app/serialise.h"
#include "app/globals.h"

#include "game/game.h"

#include "world/world.h"
#include "world/scheduler/shotbyfedsevent.h"
#include "world/generator/consequencegenerator.h"

#include "options/options.h"



ShotByFedsEvent::ShotByFedsEvent ()
{

	reason = NULL;

}

ShotByFedsEvent::~ShotByFedsEvent ()
{


}

void ShotByFedsEvent::SetName ( const char *newname )
{

	UplinkAssert ( strlen (newname) < SIZE_PERSON_NAME );
	name = newname;

}

void ShotByFedsEvent::SetReason ( const char *newreason )
{

	reason = newreason;

}

void ShotByFedsEvent::Run ()
{

	Person *person = game->GetWorld ()->GetPerson ( name.c_str () );
	UplinkAssert (person);

	person->SetStatus ( PERSON_STATUS_DEAD );

	ConsequenceGenerator::ShotByFeds ( person, reason.c_str () );

}

char *ShotByFedsEvent::GetShortString ()
{

	size_t shortstringsize = strlen(name.c_str ()) + 32;
	char *shortstring = new char [shortstringsize];
	UplinkSnprintf ( shortstring, shortstringsize, "ShotByFeds for %s", name.c_str () );
	return shortstring;

}

char *ShotByFedsEvent::GetLongString ()
{

	std::ostringstream longstring;
	longstring << "Shot By Feds Event\n"
			   << "For : " << name << "\n"
			   << "Reason : " << reason.c_str ();
	return UplinkStrdup ( longstring.str ().c_str () );

}

bool ShotByFedsEvent::Load  ( FILE *file )
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

	LoadID_END ( file );

	return true;

}

void ShotByFedsEvent::Save  ( FILE *file )
{

	SaveID ( file );

	UplinkEvent::Save ( file );

	SaveDynamicString ( name, SIZE_PERSON_NAME, file );
	SaveDynamicString ( reason, file );

	SaveID_END ( file );

}

void ShotByFedsEvent::Print ()
{

	printf ( "ShotByFedsEvent : for %s\n", name.c_str () );
	UplinkEvent::Print ();
	printf ( "Reason : %s\n", reason.c_str () );

}

const char *ShotByFedsEvent::GetID ()
{

	return "EVT_SHOT";

}

int   ShotByFedsEvent::GetOBJECTID ()
{

	return OID_SHOTBYFEDSEVENT;

}
