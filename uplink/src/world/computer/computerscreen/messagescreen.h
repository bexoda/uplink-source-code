
/*

  Message screen class object

	Shows a message and the company title

  */


#ifndef _included_messagescreen_h
#define _included_messagescreen_h

#include "app/optstring.h"

// ============================================================================

#include <stdio.h>

#include "world/computer/computerscreen/computerscreen.h"

class ComputerScreen;

// ============================================================================


class MessageScreen : public ComputerScreen  
{

public:

	int nextpage;
	OptString textmessage;
	OptString buttonmessage;
	bool mailthistome;	

public:

	MessageScreen();
	virtual ~MessageScreen();

	void SetNextPage ( int newnextpage );
	void SetTextMessage ( const char *newtextmessage );
	void SetButtonMessage ( const char *newbuttonmessage );
	void SetMailThisToMe ( bool newvalue );

	// Common functions

	bool Load  ( FILE *file );			
	void Save  ( FILE *file );			
	void Print ();						
	
	const char *GetID ();						
	int   GetOBJECTID ();				

};

#endif
