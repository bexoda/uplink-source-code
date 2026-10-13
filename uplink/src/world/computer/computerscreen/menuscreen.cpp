// MenuScreen.cpp: implementation of the MenuScreen class.
//
//////////////////////////////////////////////////////////////////////

#include "gucci.h"

#include "app/app.h"
#include "app/globals.h"
#include "app/serialise.h"

#include "world/computer/computerscreen/menuscreen.h"



//////////////////////////////////////////////////////////////////////
// Construction/Destruction
//////////////////////////////////////////////////////////////////////

MenuScreen::MenuScreen()
{

}

MenuScreen::~MenuScreen()
{
    
    DeleteLListData ( (LList <UplinkObject *> *) &options );

}

void MenuScreen::AddOption ( const char *caption, const char *tooltip, int nextpage, int security, int index )
{

	MenuScreenOption *mso = new MenuScreenOption ();
	mso->SetCaption ( caption );
	mso->SetTooltip ( tooltip );
	mso->SetNextPage ( nextpage );
	mso->SetSecurity ( security );

	if ( index == -1 )
		options.PutData ( mso );

	else
		options.PutDataAtIndex ( mso, index );

}

int MenuScreen::NumOptions  ()
{

	return options.Size ();

}

const char *MenuScreen::GetCaption ( int index )
{

	UplinkAssert ( options.ValidIndex ( index ) );

	return options.GetData (index)->caption.c_str ();

}

const char *MenuScreen::GetTooltip ( int index )
{

	UplinkAssert ( options.ValidIndex ( index ) );

	return options.GetData (index)->tooltip.c_str ();
	
}

int MenuScreen::GetNextPage ( int index )
{

	UplinkAssert ( options.ValidIndex ( index ) );

	return options.GetData (index)->nextpage;

}

int MenuScreen::GetSecurity ( int index )
{

	UplinkAssert ( options.ValidIndex ( index ) );

	return options.GetData (index)->security;

}

bool MenuScreen::Load ( FILE *file )
{

	LoadID ( file );

	if ( !ComputerScreen::Load ( file ) ) return false;

	if ( !LoadLList ( (LList <UplinkObject *> *) &options, file ) ) return false;

	LoadID_END ( file );

	return true;

}

void MenuScreen::Save ( FILE *file )
{

	SaveID ( file );

	ComputerScreen::Save ( file );

	SaveLList ( (LList <UplinkObject *> *) &options, file );

	SaveID_END ( file );

}

void MenuScreen::Print ()
{

	printf ( "MenuScreen : \n" );
	
	ComputerScreen::Print ();
	
	PrintLList ( (LList <UplinkObject *> *) &options );

}

const char *MenuScreen::GetID ()
{

	return "SCR_MENU";

}

int MenuScreen::GetOBJECTID ()
{

	return OID_MENUSCREEN;

}



// ============================================================================



MenuScreenOption::MenuScreenOption ()
{

	caption = "";
	tooltip = "";
	nextpage = -1;
	security = 10;

}

MenuScreenOption::~MenuScreenOption ()
{
}

void MenuScreenOption::SetCaption ( const char *newcaption )
{

	UplinkAssert ( strlen(newcaption) < SIZE_MENUSCREENOPTION_CAPTION );
	caption = newcaption;

}

void MenuScreenOption::SetTooltip ( const char *newtooltip )
{

	UplinkAssert ( strlen(newtooltip) < SIZE_MENUSCREENOPTION_TOOLTIP );
	tooltip = newtooltip;

}

void MenuScreenOption::SetNextPage ( int newnextpage )
{

	nextpage = newnextpage;

}

void MenuScreenOption::SetSecurity ( int newsecurity )
{

	security = newsecurity;

}

bool MenuScreenOption::Load ( FILE *file )
{

	LoadID ( file );

	if ( !LoadDynamicStringStatic ( caption, SIZE_MENUSCREENOPTION_CAPTION, file ) ) return false;
	if ( !LoadDynamicStringStatic ( tooltip, SIZE_MENUSCREENOPTION_TOOLTIP, file ) ) return false;

	if ( !FileReadData ( &nextpage, sizeof(nextpage), 1, file ) ) return false;
	if ( !FileReadData ( &security, sizeof(security), 1, file ) ) return false;

	LoadID_END ( file );

	return true;

}

void MenuScreenOption::Save ( FILE *file )
{

	SaveID ( file );

	SaveDynamicString ( caption, SIZE_MENUSCREENOPTION_CAPTION, file );
	SaveDynamicString ( tooltip, SIZE_MENUSCREENOPTION_TOOLTIP, file );

	fwrite ( &nextpage, sizeof(nextpage), 1, file );
	fwrite ( &security, sizeof(security), 1, file );

	SaveID_END ( file );

}

void MenuScreenOption::Print ()
{

	printf ( "MenuScreenOption : \n" );
	printf ( "\tNextPage = %d, Caption = %s\n", nextpage, caption.c_str () );
	printf ( "\tTooltip = %s, security = %d\n", tooltip.c_str (), security );

}

void MenuScreenOption::Update ()
{
}

const char *MenuScreenOption::GetID ()
{

	return "MNUOPT";

}

int MenuScreenOption::GetOBJECTID ()
{

	return OID_MENUSCREENOPTION;

}

