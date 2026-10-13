
#include <string.h>

#include "gucci.h"

#include "game/game.h"

#include "app/app.h"
#include "app/globals.h"
#include "app/serialise.h"

#include "world/world.h"
#include "world/company/mission.h"
#include "world/vlocation.h"
#include "world/generator/missiongenerator.h"



Mission::Mission ()
{

	details = NULL;
	fulldetails = NULL;
	duedate.reset ();
    
	whysomuchmoney = NULL;
	howsecure = NULL;
	whoisthetarget = NULL;
	
	TYPE = MISSION_NONE;
	description = "";
	employer = "";
	contact = "";

	completionA = "";
	completionB = "";
	completionC = "";
	completionD = "";
	completionE = "";

	payment = 0;
	maxpayment = 0;
	paymentmethod = MISSIONPAYMENT_AFTERCOMPLETION;

    difficulty = 0;
    minuplinkrating = -1;
    acceptrating = 0;
	npcpriority = false;

}

Mission::~Mission ()
{


	duedate.reset ();

	DeleteLListData ( &links );
	DeleteBTreeData ( &codes );

}

void Mission::SetTYPE ( int newTYPE )
{

	TYPE = newTYPE;

}

void Mission::SetCompletion ( const char *newA, const char *newB, const char *newC, char *newD, char *newE )
{

	// No need to set them if NULL is passed in - they will not be examined after all

	if ( newA ) {
		UplinkAssert ( strlen(newA) < SIZE_MISSION_COMPLETION );
		completionA = newA;
	}

	if ( newB ) {
		UplinkAssert ( strlen(newB) < SIZE_MISSION_COMPLETION );
		completionB = newB;
	}

	if ( newC ) {
		UplinkAssert ( strlen(newC) < SIZE_MISSION_COMPLETION );
		completionC = newC;
	}

	if ( newD ) {
		UplinkAssert ( strlen(newD) < SIZE_MISSION_COMPLETION );
		completionD = newD;
	}

	if ( newE ) {
		UplinkAssert ( strlen(newE) < SIZE_MISSION_COMPLETION );
		completionE = newE;
	}

}

void Mission::SetCreateDate ( Date *date )
{

	UplinkAssert ( date );
	createdate.SetDate ( date );

}

void Mission::SetNpcPriority ( bool newpriority )
{

	npcpriority = newpriority;

}

void Mission::SetDescription ( const char *newdescription )
{

	UplinkAssert ( strlen ( newdescription ) < SIZE_MISSION_DESCRIPTION );
	description = newdescription;

}

void Mission::SetDetails ( const char *newdetails )
{


	details = newdetails;

}

void Mission::SetFullDetails ( const char *newdetails )
{


	fulldetails = newdetails;

}

void Mission::SetWhySoMuchMoney	( const char *answer )
{

	whysomuchmoney = answer;

}

void Mission::SetHowSecure ( const char *answer )
{

	howsecure = answer;

}

void Mission::SetWhoIsTheTarget ( const char *answer )
{

	whoisthetarget = answer;

}

void Mission::SetEmployer ( const char *newemployer )
{

	UplinkAssert ( strlen (newemployer) < 64 );
	UplinkAssert ( game->GetWorld ()->GetCompany (newemployer) );

	employer = newemployer;

}

void Mission::SetContact ( const char *newcontact )
{

	UplinkAssert ( strlen (newcontact) < SIZE_PERSON_NAME );
	UplinkAssert ( game->GetWorld ()->GetPerson ( newcontact ) );

	contact = newcontact;

}

void Mission::SetPayment ( int newpayment, int newmaxpayment )
{

	payment = newpayment;

	if ( newmaxpayment != -1 )
		maxpayment = newmaxpayment;

	else
		maxpayment = payment;

}

void Mission::SetDifficulty ( int newdifficulty )
{

	difficulty = newdifficulty;

}

void Mission::SetMinRating ( int newrating )
{

	minuplinkrating = newrating;

}

void Mission::SetAcceptRating ( int newrating )
{

	acceptrating = newrating;

}

void Mission::GiveLink ( const char *ip )
{

	UplinkAssert ( strlen (ip) < SIZE_VLOCATION_IP );
	size_t theipsize = SIZE_VLOCATION_IP;
	char *theip = new char [theipsize];
	UplinkStrncpy ( theip, ip, theipsize );
	links.PutData ( theip );

}

void Mission::GiveCode ( const char *ip, char *code )
{

	UplinkAssert ( strlen (ip) < SIZE_VLOCATION_IP );
	char theip [SIZE_VLOCATION_IP];
	UplinkStrncpy ( theip, ip, sizeof ( theip ) );

	char *thecode = new char [strlen(code)+1];
	UplinkSafeStrcpy ( thecode, code );

	codes.PutData ( theip, thecode );

}

void Mission::SetDueDate ( Date *newdate )
{

	if ( !duedate )
		duedate.reset ( new Date() );

	duedate->SetDate( newdate );

}

Date *Mission::GetDueDate ()
{

	return duedate.get ();

}

const char *Mission::GetDetails ()
{

	UplinkAssert ( details );
	return details.c_str ();

}

const char *Mission::GetFullDetails ()
{

	UplinkAssert ( fulldetails );
	return fulldetails.c_str ();

}

const char *Mission::GetWhySoMuchMoney ()
{

	return whysomuchmoney.c_str ();

}

const char *Mission::GetHowSecure	()
{

	return howsecure.c_str ();

}

const char *Mission::GetWhoIsTheTarget ()
{

	return whoisthetarget.c_str ();

}

bool Mission::Load  ( FILE *file )
{

	LoadID ( file );

	duedate.reset ();

	if ( !FileReadData ( &TYPE, sizeof(TYPE), 1, file ) ) return false;
	
	if ( !LoadDynamicStringStatic ( completionA, SIZE_MISSION_COMPLETION, file ) ) return false;
	if ( !LoadDynamicStringStatic ( completionB, SIZE_MISSION_COMPLETION, file ) ) return false;
	if ( !LoadDynamicStringStatic ( completionC, SIZE_MISSION_COMPLETION, file ) ) return false;
	if ( !LoadDynamicStringStatic ( completionD, SIZE_MISSION_COMPLETION, file ) ) return false;
	if ( !LoadDynamicStringStatic ( completionE, SIZE_MISSION_COMPLETION, file ) ) return false;

	if ( !createdate.Load ( file ) ) return false;

	if ( !LoadDynamicStringStatic ( employer, SIZE_MISSION_EMPLOYER, file ) ) return false;
	if ( !LoadDynamicStringStatic ( contact, SIZE_PERSON_NAME, file ) ) return false;
	if ( !LoadDynamicStringStatic ( description, SIZE_MISSION_DESCRIPTION, file ) ) return false;

	if ( !FileReadData ( &payment, sizeof(payment), 1, file ) ) return false;
	if ( !FileReadData ( &difficulty, sizeof(difficulty), 1, file ) ) return false;
	if ( !FileReadData ( &minuplinkrating, sizeof(minuplinkrating), 1, file ) ) return false;
	if ( !FileReadData ( &acceptrating, sizeof(acceptrating), 1, file ) ) return false;
	if ( !FileReadData ( &npcpriority, sizeof(npcpriority), 1, file) ) return false;
	
	if ( !LoadDynamicStringPtr ( &details, file ) ) return false;
	if ( !LoadDynamicStringPtr ( &fulldetails, file ) ) return false;
	if ( !LoadDynamicStringPtr ( &whysomuchmoney, file ) ) return false;
	if ( !LoadDynamicStringPtr ( &howsecure, file ) ) return false;
	if ( !LoadDynamicStringPtr ( &whoisthetarget, file ) ) return false;
	
	if ( !LoadLList ( &links, file ) ) return false;
	if ( !LoadBTree ( &codes, file ) ) return false;

	bool hasduedate;
	if ( !FileReadData ( &hasduedate, sizeof(hasduedate), 1, file ) ) return false;

	if ( hasduedate ) {
		duedate.reset ( new Date () );
		if ( !duedate->Load ( file ) ) return false;
	}

	if ( strcmp( game->GetLoadedSavefileVer(), "SAV57" ) >= 0 ) {

		if ( !FileReadData ( &maxpayment, sizeof(maxpayment), 1, file ) ) return false;
		if ( !FileReadData ( &paymentmethod, sizeof(paymentmethod), 1, file ) ) return false;

	}

	LoadID_END ( file );

	return true;

}

void Mission::Save  ( FILE *file )
{

	SaveID ( file );

	fwrite ( &TYPE, sizeof(TYPE), 1, file );
	
	SaveDynamicString ( completionA, SIZE_MISSION_COMPLETION, file );
	SaveDynamicString ( completionB, SIZE_MISSION_COMPLETION, file );
	SaveDynamicString ( completionC, SIZE_MISSION_COMPLETION, file );
	SaveDynamicString ( completionD, SIZE_MISSION_COMPLETION, file );
	SaveDynamicString ( completionE, SIZE_MISSION_COMPLETION, file );

	createdate.Save ( file );

	SaveDynamicString ( employer, SIZE_MISSION_EMPLOYER, file );
	SaveDynamicString ( contact, SIZE_PERSON_NAME, file );
	SaveDynamicString ( description, SIZE_MISSION_DESCRIPTION, file );

	fwrite ( &payment, sizeof(payment), 1, file );
	fwrite ( &difficulty, sizeof(difficulty), 1, file );
	fwrite ( &minuplinkrating, sizeof(minuplinkrating), 1, file );
	fwrite ( &acceptrating, sizeof(acceptrating), 1, file );
	fwrite ( &npcpriority, sizeof(npcpriority), 1, file );
	
	SaveDynamicString ( details, file );
	SaveDynamicString ( fulldetails, file );
	SaveDynamicString ( whysomuchmoney, file );
	SaveDynamicString ( howsecure, file );
	SaveDynamicString ( whoisthetarget, file );

	SaveLList ( &links, file );
	SaveBTree ( &codes, file );

	bool hasduedate = (duedate != NULL);
	fwrite ( &hasduedate, sizeof(hasduedate), 1, file );
	if ( hasduedate ) 	duedate->Save ( file );

	fwrite ( &maxpayment, sizeof(maxpayment), 1, file );
	fwrite ( &paymentmethod, sizeof(paymentmethod), 1, file );
	
	SaveID_END ( file );

}

void Mission::Print ()
{

	printf ( "Mission : TYPE=%d\n", TYPE );
	printf ( "Employer=%s, Payment=%d, Difficulty=%d, MinRating=%d, AcceptRating=%d, Description=%s\n",
			 employer.c_str (), payment, difficulty, minuplinkrating, acceptrating, description.c_str () );
	printf ( "NPC priority = %d\n", npcpriority );
	createdate.Print ();
	printf ( "\tCompletionA = %s\n", completionA.c_str () );
	printf ( "\tCompletionB = %s\n", completionB.c_str () );
	printf ( "\tCompletionC = %s\n", completionC.c_str () );
	printf ( "\tCompletionD = %s\n", completionD.c_str () );
	printf ( "\tCompletionE = %s\n", completionE.c_str () );
	printf ( "\tContact = %s\n", contact.c_str () );
	printf ( "\tDetails=%s\n", details.c_str () );
	printf ( "\tFullDetails=%s\n", fulldetails.c_str () );
	printf ( "\tWhySoMuchMoney=%s\n", whysomuchmoney.c_str () );
	printf ( "\tHowSecure=%s\n", howsecure.c_str () );

	PrintLList ( &links );
	PrintBTree ( &codes );

	if ( duedate ) {
		printf ( "Due date :\n" );
		duedate->Print ();
	}
	else 
		printf ( "No due date\n" );

}
	
const char *Mission::GetID ()
{

	return "MISSION";

}

int Mission::GetOBJECTID ()
{

	return OID_MISSION;

}

