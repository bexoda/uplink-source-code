// ComputerScreen.cpp: implementation of the ComputerScreen class.
//
//////////////////////////////////////////////////////////////////////


#include "game/game.h"

#include "app/app.h"
#include "app/globals.h"
#include "app/serialise.h"

#include "world/world.h"
#include "world/computer/computerscreen/computerscreen.h"




ComputerScreen::ComputerScreen()
{

	maintitle = " ";
	subtitle = " ";
	computer = " ";

}

ComputerScreen::~ComputerScreen()
{

}

void ComputerScreen::SetMainTitle ( const char *newmaintitle )
{

	UplinkAssert ( strlen(newmaintitle) < SIZE_COMPUTERSCREEN_MAINTITLE );
	maintitle = newmaintitle;

}

void ComputerScreen::SetSubTitle ( const char *newsubtitle )
{

	UplinkAssert ( strlen(newsubtitle) < SIZE_COMPUTERSCREEN_SUBTITLE );
	subtitle = newsubtitle;

}

void ComputerScreen::SetComputer ( const char *newcomputer )
{

	UplinkAssert ( strlen(newcomputer) < SIZE_COMPUTER_NAME );
	computer = newcomputer;

}

Computer *ComputerScreen::GetComputer ()
{

	return game->GetWorld ()->GetComputer ( computer.c_str () );

}

bool ComputerScreen::Load  ( FILE *file )
{

	LoadID ( file );

	if ( !LoadDynamicStringStatic ( maintitle, SIZE_COMPUTERSCREEN_MAINTITLE, file ) ) return false;
	if ( !LoadDynamicStringStatic ( subtitle, SIZE_COMPUTERSCREEN_SUBTITLE, file ) ) return false;
	if ( !LoadDynamicStringStatic ( computer, SIZE_COMPUTER_NAME, file ) ) return false;

	LoadID_END ( file );

	return true;

}

void ComputerScreen::Save  ( FILE *file )
{

	SaveID ( file );

	SaveDynamicString ( maintitle, SIZE_COMPUTERSCREEN_MAINTITLE, file );
	SaveDynamicString ( subtitle, SIZE_COMPUTERSCREEN_SUBTITLE, file );
	SaveDynamicString ( computer, SIZE_COMPUTER_NAME, file );

	SaveID_END ( file );

}

void ComputerScreen::Print ()
{

	printf ( "MainTitle : %s\n", maintitle.c_str () );
	printf ( "SubTitle  : %s\n", subtitle.c_str () );

	printf ( "Computer  : %s\n", computer.c_str () );

}

void ComputerScreen::Update ()
{
}

const char *ComputerScreen::GetID ()
{

	return "COMPSCR";

}

int ComputerScreen::GetOBJECTID ()
{
	
	return OID_UNKNOWN;

}

