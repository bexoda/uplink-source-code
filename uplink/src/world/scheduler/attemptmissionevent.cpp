
#include <sstream>


#include "app/app.h"
#include "app/globals.h"
#include "app/serialise.h"

#include "game/game.h"

#include "world/world.h"
#include "world/agent.h"
#include "world/company/mission.h"
#include "world/scheduler/attemptmissionevent.h"



AttemptMissionEvent::AttemptMissionEvent ()
{


}

AttemptMissionEvent::~AttemptMissionEvent ()
{
}

void AttemptMissionEvent::Run ()
{

	Agent *agent = (Agent *) game->GetWorld ()->GetPerson ( agentname.c_str () );
	UplinkAssert (agent);

	agent->AttemptMission ();

}

void AttemptMissionEvent::SetAgentName ( const char *newagentname )
{

	UplinkAssert ( strlen (newagentname) < SIZE_PERSON_NAME );
	agentname = newagentname;

}

char *AttemptMissionEvent::GetShortString ()
{

	size_t shortstringsize = strlen(agentname.c_str ()) + 32;
	char *shortstring = new char [shortstringsize];
	UplinkSnprintf ( shortstring, shortstringsize, "Attempt Mission for %s", agentname.c_str () );
	return shortstring;

}

char *AttemptMissionEvent::GetLongString ()
{

	Agent *agent = (Agent *) game->GetWorld ()->GetPerson ( agentname.c_str () );
	UplinkAssert (agent);

	Mission *m = agent->missions.GetData (0);
	UplinkAssert (m);

	std::ostringstream longstring;
	longstring << "Attempt Mission Event:\n"
			   << "Agent : " << agentname << "\n"
			   << "Agents next mission : " << m->description;

	return UplinkStrdup ( longstring.str ().c_str () );

}


bool AttemptMissionEvent::Load ( FILE *file )
{

	LoadID ( file );

	if ( !UplinkEvent::Load ( file ) ) return false;

	if ( strcmp( game->GetLoadedSavefileVer(), "SAV59" ) >= 0 ) {
		if ( !LoadDynamicStringStatic ( agentname, SIZE_PERSON_NAME, file ) ) return false;
	}
	else {
		if ( !LoadFixedString ( agentname, SIZE_PERSON_NAME, file ) ) return false;
	}

	LoadID_END ( file );

	return true;

}

void AttemptMissionEvent::Save ( FILE *file )
{

	SaveID ( file );

	UplinkEvent::Save ( file );

	SaveDynamicString ( agentname, SIZE_PERSON_NAME, file );

	SaveID_END ( file );

}

void AttemptMissionEvent::Print ()
{

	printf ( "AttemptMissionEvent : agentname=%s\n", agentname.c_str () );

	UplinkEvent::Print ();

}

const char *AttemptMissionEvent::GetID ()
{

	return "EVT_AME";

}

int AttemptMissionEvent::GetOBJECTID ()
{

	return OID_ATTEMPTMISSIONEVENT;

}
