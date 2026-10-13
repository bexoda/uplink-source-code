
/*

  Shot By Feds event object

	Occurs when someone is shot by federal agents

  */


#ifndef _included_shotbyfedsevent_h
#define _included_shotbyfedsevent_h

#include "app/optstring.h"

#include <string>

// ============================================================================


#include "world/person.h"
#include "world/scheduler/uplinkevent.h"

// ============================================================================



class ShotByFedsEvent : public UplinkEvent
{

protected:

	OptString reason;

public:

	std::string name;

public:

	ShotByFedsEvent ();
	~ShotByFedsEvent ();

	void SetName ( const char *newname );
	void SetReason ( const char *newreason );

	void Run ();

	char *GetShortString ();
	char *GetLongString ();

	// Common functions

	bool Load  ( FILE *file );
	void Save  ( FILE *file );
	void Print ();
	
	const char *GetID ();		
	int   GetOBJECTID ();

};


#endif

