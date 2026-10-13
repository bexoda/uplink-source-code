
#include <sstream>

#include "soundgarden.h"

#include "app/app.h"
#include "app/globals.h"
#include "app/serialise.h"

#include "game/game.h"

#include "world/world.h"
#include "world/vlocation.h"
#include "world/generator/consequencegenerator.h"
#include "world/scheduler/arrestevent.h"

#include "options/options.h"




ArrestEvent::ArrestEvent ()
{

	reason = NULL;
	ip = NULL;

}

ArrestEvent::~ArrestEvent ()
{


}

void ArrestEvent::SetName ( const char *newname )
{

	UplinkAssert ( strlen(newname) < SIZE_PERSON_NAME );
	name = newname;

}

void ArrestEvent::SetReason ( const char *newreason )
{

	reason = newreason;

}

void ArrestEvent::SetIP ( const char *newip )
{

	ip = newip;

}

void ArrestEvent::Run ()
{

	Person *person = game->GetWorld ()->GetPerson ( name.c_str () );
	UplinkAssert (person);

	person->SetStatus ( PERSON_STATUS_INJAIL );

	// Look up the computer they messed with

	Computer *comp = NULL;

	if ( ip ) {
		VLocation *vl = game->GetWorld ()->GetVLocation ( ip.c_str () );
		UplinkAssert (vl);
		comp = vl->GetComputer ();
		UplinkAssert (comp);
	}

	ConsequenceGenerator::Arrested ( person, comp, reason.c_str () );

}

char *ArrestEvent::GetShortString ()
{

	size_t shortstringsize = strlen(name.c_str ()) + 16;
	char *shortstring = new char [shortstringsize];
	UplinkSnprintf ( shortstring, shortstringsize, "Arrest for %s", name.c_str () );
	return shortstring;
}

char *ArrestEvent::GetLongString ()
{

	std::ostringstream longstring;
	longstring << "ArrestEvent:\n"
			   << "For : " << name << "\n"
			   << "Reason : " << reason.c_str ();

	return UplinkStrdup ( longstring.str ().c_str () );

}


bool ArrestEvent::Load  ( FILE *file )
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
	if ( !LoadDynamicStringPtr ( &ip, file) ) return false;

	LoadID_END ( file );

	return true;

}

void ArrestEvent::Save  ( FILE *file )
{

	SaveID ( file );

	UplinkEvent::Save ( file );

	SaveDynamicString ( name, SIZE_PERSON_NAME, file );

	SaveDynamicString ( reason, file );
	SaveDynamicString ( ip, file );

	SaveID_END ( file );

}

void ArrestEvent::Print ()
{

	printf ( "Arrest Event for %s:\n", name.c_str () );
	UplinkEvent::Print ();

	printf ( "%s\n", reason.c_str () );
	if ( ip ) printf ( "IP:%s\n", ip.c_str () );


}

const char *ArrestEvent::GetID ()
{

	return "EVT_ARR";

}

int   ArrestEvent::GetOBJECTID ()
{

	return OID_ARRESTEVENT;

}

