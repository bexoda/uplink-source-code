
/*

  Message class object

	Stores the data representing a message (system message, email)

  */


#ifndef _included_message_h
#define _included_message_h

#include <memory>

#include "app/optstring.h"

#include <string>

// ============================================================================

#include "app/uplinkobject.h"

#include "world/person.h"
#include "world/date.h"

class Data;

// ============================================================================


class Message : public UplinkObject  
{

protected:

	OptString subject;
	OptString body;

	std::unique_ptr<Data> data;							// An attached file/program

public:

	std::string to;
	std::string from;

	LList <char *> links;				// All useful ip addresses (to be added to links)
	BTree <char *> codes;				// Access codes, indexed on ip

	Date date;							// Date of send

public:

	Message();
	virtual ~Message();

	void SetTo		( const char *newto );
	void SetFrom	( const char *newfrom );
	void SetSubject ( const char *newsubject );
	void SetBody	( const char *newbody );
	void SetDate    ( Date *newdate );

	void GiveLink ( const char *ip );
	void GiveCode ( const char *ip, char *code );

	void AttachData ( Data *newdata );
	Data *GetData ();

	const char *GetSubject ();
	const char *GetBody	 ();

	void Send ();


	// Common functions

	bool Load  ( FILE *file );
	void Save  ( FILE *file );
	void Print ();
	
	const char *GetID ();
	int   GetOBJECTID ();

};

#endif
