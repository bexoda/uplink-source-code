
/*

  Seize Gateway event

	Occurs when federal agents decide to grab your gateway
	(Only applies to Uplink Agents)

  */

#ifndef _included_seizeevent_h
#define _included_seizeevent_h

#include "app/optstring.h"

#include <string>


#include "world/person.h"
#include "world/scheduler/uplinkevent.h"


class SeizeGatewayEvent : public UplinkEvent
{

protected:

	OptString reason;

public:

	std::string name;
	int gateway_id;							// id number of gateway to seize

public:

	SeizeGatewayEvent ();
	~SeizeGatewayEvent ();

	void SetName ( const char *newname );
	void SetReason ( const char *newreason );
	void SetGatewayID ( int newid );

	void Run ();
	void RunWarning ();

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

