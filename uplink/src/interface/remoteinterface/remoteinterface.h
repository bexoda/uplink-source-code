
/*

  Remote Interface class object

	Part of the Interface sub-system
	Handles the interface projected by the current host machine -
	 for example a password screen, a log screen etc
	
  */

#ifndef _included_remoteinterface_h
#define _included_remoteinterface_h

#include <memory>

#include <string>


#include "app/uplinkobject.h"

#include "interface/remoteinterface/remoteinterfacescreen.h"

#include "world/person.h"

#include "world/computer/computerscreen/screentypes.h"


class Computer;


class RemoteInterface : public UplinkObject
{

protected:

	std::unique_ptr<RemoteInterfaceScreen> screen;

public:

	int previousscreenindex;
	int currentscreenindex;
	std::string security_name;
	int  security_level;

public:

	RemoteInterface ();
	~RemoteInterface ();

	void RunNewLocation ();						// Looks at players location
	
	void RunScreen ( int screenindex, Computer *compref = NULL );			// Runs at existing location
	bool VerifyScreen ( int screenindex );

	void SetSecurity ( const char *newname, int newvalue );

	void Create();
	void Remove ();
	bool IsVisible ();

	RemoteInterfaceScreen *GetInterfaceScreen ();			// Asserts screen
	ComputerScreen *GetComputerScreen ();					

	// Common functions

	bool Load   ( FILE *file );
	void Save   ( FILE *file );
	void Print  ();
	void Update ();

	const char *GetID ();

};


#endif
