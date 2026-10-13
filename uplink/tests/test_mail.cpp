// Sending mail from the player's Send Mail screen. The interface builds the message and
// hands it to the recipient on Send: the recipient owns it from then on, so the world must
// still save it and the interface must start a fresh message.

#include <catch2/catch_test_macros.hpp>

#include <string>

#include "harness.h"

#include "app/globals.h"
#include "game/game.h"
#include "interface/interface.h"
#include "interface/localinterface/localinterface.h"
#include "world/world.h"
#include "world/player.h"
#include "world/message.h"
#include "eclipse.h"

TEST_CASE ( "a mail sent from the Send Mail screen reaches the recipient and is saved", "[mail]" )
{
	harness::NewWorld ( 7 );
	Player *player = game->GetWorld ()->GetPlayer ();
	int before = player->messages.Size ();

	LocalInterface *li = game->GetInterface ()->GetLocalInterface ();
	li->RunScreen ( SCREEN_SENDMAIL );
	REQUIRE ( EclGetButton ( "sendmail_send" ) );

	EclGetButton ( "sendmail_to" )->SetCaption ( player->name.c_str () );
	EclGetButton ( "sendmail_subject" )->SetCaption ( "Test subject" );
	EclGetButton ( "sendmail_body box" )->SetCaption ( "Test body" );
	EclGetButton ( "sendmail_send" )->MouseUp ();

	REQUIRE ( player->messages.Size () == before + 1 );
	Message *sent = player->messages.GetData ( player->messages.Size () - 1 );
	CHECK ( std::string ( sent->GetSubject () ) == "Test subject" );
	CHECK ( std::string ( sent->GetBody () ) == "Test body" );

	// A second mail must be a new message, not the one now in the inbox. Sending closed the screen.
	li->RunScreen ( SCREEN_SENDMAIL );
	EclGetButton ( "sendmail_to" )->SetCaption ( player->name.c_str () );
	EclGetButton ( "sendmail_subject" )->SetCaption ( "Second" );
	EclGetButton ( "sendmail_send" )->MouseUp ();
	REQUIRE ( player->messages.Size () == before + 2 );
	CHECK ( std::string ( sent->GetSubject () ) == "Test subject" );

	li->RunScreen ( SCREEN_NONE );
	CHECK ( harness::SaveBytes ().size () > 0 );		// walks every message
}
