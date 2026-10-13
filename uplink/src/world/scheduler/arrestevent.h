
/*

  Arrest event 

	Occurs when a person is arrested by federal agents
	(usually for hacking)

  */

#ifndef _included_arrestevent_h
#define _included_arrestevent_h

#include "app/optstring.h"

#include <string>


#include "world/person.h"
#include "world/scheduler/uplinkevent.h"


class ArrestEvent : public UplinkEvent
{

protected:

	OptString reason;
	OptString ip;							

public:

	std::string name;

public:

	ArrestEvent ();
	~ArrestEvent ();

	void SetName	( const char *newname );
	void SetReason	( const char *newreason );
	void SetIP		( const char *newip );

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

