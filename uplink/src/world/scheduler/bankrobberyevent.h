
/*

  Bank Robbery Event

    Created when the player attempts to transfer money 
    into his own bank account

    Runs roughly two minutes later

    If the player hasn't covered his tracks, its game over

  */



#ifndef _included_bankrobberyevent_h
#define _included_bankrobberyevent_h

#include <string>

#include "world/scheduler/eventscheduler.h"


class BankRobberyEvent : public UplinkEvent
{

public:

    std::string source_ip;
    std::string source_accno;
    std::string target_ip;
    std::string target_accno;
    int amount;
    Date hackdate;

public:

    BankRobberyEvent ();
    ~BankRobberyEvent ();

	void Run ();

    void SetDetails ( const char *newsourceip, char *newsourceaccno,
                      const char *newtargetip, char *newtargetaccno,
                      int newamount, Date *newhackdate );

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
