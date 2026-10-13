// Message.cpp: implementation of the Message class.
//
//////////////////////////////////////////////////////////////////////


#include "game/game.h"

#include "app/app.h"
#include "app/globals.h"
#include "app/serialise.h"

#include "world/world.h"
#include "world/message.h"
#include "world/computer/databank.h"


//////////////////////////////////////////////////////////////////////
// Construction/Destruction
//////////////////////////////////////////////////////////////////////

Message::Message()
{
	
	subject = NULL;
	body = NULL;

	SetTo ( " " );
	SetFrom ( " " );
	SetSubject ( " " );
	SetBody ( " " );

	data.reset ();

}
 
Message::~Message()
{

	data.reset ();

    DeleteLListData ( &links );
    DeleteBTreeData ( &codes );

}

void Message::SetTo	( const char *newto )
{

	UplinkAssert ( strlen (newto) < SIZE_PERSON_NAME );
	to = newto;

}

void Message::SetFrom ( const char *newfrom )
{

	UplinkAssert ( strlen (newfrom) < SIZE_PERSON_NAME );
	from = newfrom;

}

void Message::SetSubject ( const char *newsubject )
{

	subject = newsubject;

}

void Message::SetBody ( const char *newbody )
{

	body = newbody;

}

void Message::SetDate ( Date *newdate )
{

	date.SetDate ( newdate );

}

void Message::GiveLink ( const char *ip )
{

	size_t theipsize = SIZE_VLOCATION_IP;
	char *theip = new char [theipsize];
	UplinkAssert (strlen(ip) < SIZE_VLOCATION_IP );
	UplinkStrncpy ( theip, ip, theipsize );
	links.PutData (theip);

}

void Message::GiveCode ( const char *ip, char *code )
{

	char *thecode = new char [strlen(code)+1];
	UplinkSafeStrcpy ( thecode, code );
    codes.PutData ( ip, thecode );

}

void Message::AttachData ( Data *newdata )
{

	data.reset ();
	// Copy newdata
	data.reset ( new Data ( newdata ) );	

}

Data *Message::GetData ()
{

	return data.get ();

}

const char *Message::GetSubject ()
{

	UplinkAssert ( subject );
	return subject.c_str ();

}

const char *Message::GetBody ()
{

	UplinkAssert ( body );
	return body.c_str ();

}

void Message::Send ()
{

	SetDate ( &(game->GetWorld ()->date) );

	Person *person = game->GetWorld ()->GetPerson ( to.c_str () );

	if ( person ) {

		person->GiveMessage ( this );

	}
	else {

		// Send an error message to the sender
		Message *msg = new Message ();
		msg->SetTo ( from.c_str () );
		msg->SetFrom ( "MAIL SERVER" );
		msg->SetSubject ( "Failed to deliver this email - unknown recipient" );
		msg->SetBody ( body.c_str () );
		msg->Send ();

	}

}

bool Message::Load ( FILE *file )
{

	LoadID ( file );

	if ( !LoadDynamicStringStatic ( to, SIZE_PERSON_NAME, file ) ) return false;
	if ( !LoadDynamicStringStatic ( from, SIZE_PERSON_NAME, file ) ) return false;

	if ( !LoadDynamicStringPtr ( &subject, file ) ) return false;
	if ( !LoadDynamicStringPtr ( &body, file ) ) return false;

	if ( !date.Load ( file ) ) return false;

	if ( !LoadLList ( &links, file ) ) return false;
	if ( !LoadBTree ( &codes, file ) ) return false;

	LoadID_END ( file );

	return true;

}

void Message::Save ( FILE *file )
{

	SaveID ( file );

	SaveDynamicString ( to, SIZE_PERSON_NAME, file );
	SaveDynamicString ( from, SIZE_PERSON_NAME, file );

	SaveDynamicString ( subject, file );
	SaveDynamicString ( body, file );

	date.Save ( file );

	SaveLList ( &links, file );
	SaveBTree ( &codes, file );

	SaveID_END ( file );

}

void Message::Print ()
{

	printf ( "Message : \n" );
	printf ( "\tTo   = %s\n", to.c_str () );
	printf ( "\tFrom = %s\n", from.c_str () );
	printf ( "\tSubject = %s\n", subject.c_str () );
	printf ( "\tbody = %s\n", body.c_str () );

	PrintLList ( &links );
	PrintBTree ( &codes );

	date.Print ();

}
	
const char *Message::GetID ()
{

	return "MESSAGE";

}

int Message::GetOBJECTID ()
{

	return OID_MESSAGE;

}
