
/*

   Mission class object
   Part of the World sub-system

   A representation of a mission or task to be completed
	
	*/


#ifndef _included_mission_h
#define _included_mission_h

#include "app/optstring.h"

#include <string>

// ============================================================================

#include "app/uplinkobject.h"
#include "world/date.h"

#define SIZE_MISSION_DESCRIPTION    128
#define SIZE_MISSION_EMPLOYER       64
#define SIZE_MISSION_COMPLETION     64

#define SIZE_PERSON_NAME	 128

#define MISSIONPAYMENT_AFTERCOMPLETION		0
#define MISSIONPAYMENT_HALFATSTART			1
#define MISSIONPAYMENT_ALLATSTART			2

// ============================================================================


class Mission : public UplinkObject
{

protected:

	OptString details;						// Listed on bbs boards
	OptString fulldetails;					// Listed after acceptance (contains actual target details)
	Date *duedate;						// Can be NULL

	OptString whysomuchmoney;				// Answer to these questions
	OptString howsecure;					// They can be NULL
	OptString whoisthetarget;				//

public:

	int TYPE;												// Defined in MissionGenerator
	std::string employer;
	std::string contact;
	std::string description;

	Date createdate;

	std::string completionA;				// String used to determine if this mission is done
	std::string completionB;				// String used to determine if this mission is done
	std::string completionC;				// String used to determine if this mission is done
	std::string completionD;				// String used to determine if this mission is done
	std::string completionE;				// String used to determine if this mission is done

	int payment;
	int maxpayment;						// You can negotiate this if your rating is high enough
	int paymentmethod;					// Eg all now, half now, none now

	int difficulty;
	int minuplinkrating;				// If you have this rating you can view this mission
	int acceptrating;					// If you have this rating you can auto-accept this mission

	bool npcpriority;					// NPC's will take this one first if this bool is true

	LList <char *> links;				// All useful ip addresses (to be added to links)
	BTree <char *> codes;				// Access codes, indexed on ip

public:

	Mission ();
	~Mission ();

	void SetTYPE			( int newTYPE );
	void SetCompletion		( const char *newA, const char *newB, const char *newC, char *newD, char *newE );

	void SetCreateDate		( Date *date );
	void SetEmployer		( const char *newemployer );
	void SetContact			( const char *newcontact );
	void SetPayment			( int newpayment, int newmaxpayment = -1 );
	void SetDifficulty		( int newdifficulty );
	void SetMinRating		( int newrating );
	void SetAcceptRating	( int newrating );
	void SetNpcPriority		( bool newpriority );
	void SetDescription		( const char *newdescription );
	void SetDetails			( const char *newdetails );
	void SetFullDetails		( const char *newdetails );
	
	void SetWhySoMuchMoney	( const char *answer );
	void SetHowSecure		( const char *answer );
	void SetWhoIsTheTarget  ( const char *answer );

	void GiveLink			( const char *ip );
	void GiveCode			( const char *ip, char *code );

	void SetDueDate			( Date *newdate );
	Date *GetDueDate		();							// Can return NULL

	const char *GetDetails		();
	const char *GetFullDetails	();
	const char *GetWhySoMuchMoney ();							// Can return NULL
	const char *GetHowSecure		();							// Can return NULL
	const char *GetWhoIsTheTarget ();							// Can return NULL

	// Common functions

	bool Load  ( FILE *file );
	void Save  ( FILE *file );
	void Print ();
	
	const char *GetID ();
	int   GetOBJECTID ();

};


#endif
