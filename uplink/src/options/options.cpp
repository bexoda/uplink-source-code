// -*- tab-width:4; c-file-mode:"cc-mode" -*-
// Options.cpp: implementation of the Options class.
//
//////////////////////////////////////////////////////////////////////

#include <sstream>

#include <fstream>
#include "app/dos2unix.h"

#include "gucci.h"
#include "redshirt.h"

#include "app/globals.h"
#include "app/app.h"
#include "app/miscutils.h"
#include "app/serialise.h"
#include "app/opengl.h"

#include "options/options.h"


//////////////////////////////////////////////////////////////////////
// Construction/Destruction
//////////////////////////////////////////////////////////////////////

static ColourOption getColourDefault ( 0.0, 0.0, 0.0 );


Options::Options()
{
    themeName = "graphics";
}

Options::~Options()
{

	DeleteBTreeData ( (BTree <UplinkObject *> *) &options );


	DArray <ColourOption *> *cols = colours.ConvertToDArray ();

	for ( int i = 0; i < cols->Size (); ++i )
		if ( cols->ValidIndex (i) )
			if ( cols->GetData (i) )
				delete cols->GetData (i);

	delete cols;

}

Option *Options::GetOption ( const char *name )
{

	BTree <Option *> *btree = options.LookupTree ( name );

	if ( btree )
		return btree->data;

	else
		return NULL;

}

int Options::GetOptionValue ( const char *name )
{

	Option *option = GetOption ( name );

	if ( !option ) {
		char msg [256];
		UplinkSnprintf ( msg, sizeof ( msg ), "Option %s not found", name );
		UplinkAbort(msg);
	}

	return option->value;

}

bool Options::IsOptionEqualTo ( const char *name, int value )
{

	Option *option = GetOption ( name );

	if ( option && option->value == value )
		return true;

	else
		return false;

}

void Options::SetOptionValue ( const char *name, int newvalue )
{

	BTree <Option *> *btree = options.LookupTree ( name );

	if ( btree ) {

		UplinkAssert ( btree->data );
		Option *option = btree->data;
		option->SetValue ( newvalue );

	}
	else {

		printf ( "Tried to set unrecognised option: %s\n", name );

	}

}

void Options::SetOptionValue ( const char *name, int newvalue, const char *tooltip, bool yesorno, bool visible )
{

	BTree <Option *> *btree = options.LookupTree ( name );

	if ( btree ) {

		UplinkAssert ( btree->data );
		Option *option = btree->data;
		option->SetValue ( newvalue );
		option->SetTooltip ( tooltip );
		option->SetYesOrNo ( yesorno );
		option->SetVisible ( visible );

	}
	else {

		// Create a new entry
		Option *option = new Option ();
		option->SetName ( name );
		option->SetValue ( newvalue );
		option->SetTooltip ( tooltip );
		option->SetYesOrNo ( yesorno );
		option->SetVisible ( visible );

		options.PutData ( name, option );

	}


}

LList <Option *> *Options::GetAllOptions ( const char *searchstring, bool returnhidden )
{

	DArray <Option *> *alloptions = options.ConvertToDArray ();
	LList <Option *> *result = new LList <Option *> ();

	for ( int i = 0; i < alloptions->Size (); ++i ) {
		if ( alloptions->ValidIndex (i) ) {

			Option *option = alloptions->GetData (i);
			UplinkAssert (option);

			if ( option->visible || returnhidden )
				if ( !searchstring || strstr ( option->name.c_str (), searchstring ) != NULL )
					result->PutData ( option );

		}
	}

	delete alloptions;
	return result;

}

void Options::CreateDefaultOptions ()
{

	// Game

	if ( !GetOption ( "game_debugstart" ) )				SetOptionValue ( "game_debugstart", 1, "z", true, false );

#ifndef TESTGAME
	if ( !GetOption ( "game_firsttime" ) ) {
		DArray <char *> *existing = App::ListExistingGames ();
		int lenexisting = existing->Size ();

		if ( lenexisting > 0 ) {
			SetOptionValue ( "game_firsttime", 0, "z", true, false );
		}
		else {
			SetOptionValue ( "game_firsttime", 1, "z", true, false );
		}

		for ( int i = 0; i < lenexisting; i++ )
			if ( existing->ValidIndex ( i ) )
				delete [] existing->GetData (i);

		delete existing;
	}
#else
	if ( !GetOption ( "game_firsttime" ) )				SetOptionValue ( "game_firsttime", 0, "z", true, false );
#endif

    float currentVersion;
    sscanf ( VERSION_NUMBER, "%f", &currentVersion );
    currentVersion *= 100;

    if ( !GetOption ( "game_version" ) )                SetOptionValue ( "game_version", (int) currentVersion, "z", false, false );

	// Graphics

	if ( !GetOption ( "graphics_screenwidth" ) )		SetOptionValue ( "graphics_screenwidth", 1024, "Sets the width of the screen", false, false );
	if ( !GetOption ( "graphics_screenheight" ) )		SetOptionValue ( "graphics_screenheight", 768, "Sets the height of the screen", false, false );
    if ( !GetOption ( "graphics_screendepth" ) )        SetOptionValue ( "graphics_screendepth", -1, "Sets the colour depth. -1 Means use desktop colour depth.", false, false );
    if ( !GetOption ( "graphics_screenrefresh" ) )      SetOptionValue ( "graphics_screenrefresh", -1, "Sets the refresh rate. -1 Means use desktop refresh.", false, false );
	if ( !GetOption ( "graphics_fullscreen" ) )			SetOptionValue ( "graphics_fullscreen", 1, "Sets the game to run fullscreen or in a window", true, true );
	if ( !GetOption ( "graphics_buttonanimations" ) )	SetOptionValue ( "graphics_buttonanimations", 1, "Enables or disables button animations", true, true );
	if ( !GetOption ( "graphics_safemode" ) )			SetOptionValue ( "graphics_safemode", 0, "Enables graphical safemode for troubleshooting", true, true );
	if ( !GetOption ( "graphics_softwaremouse" ) )		SetOptionValue ( "graphics_softwaremouse", 0, "Render a software mouse.  Use to correct mouse problems.", true, true );
	if ( !GetOption ( "graphics_fasterbuttonanimations" ) )	SetOptionValue ( "graphics_fasterbuttonanimations", 0, "Increase the speed of button animations.", true, true );
	if ( !GetOption ( "graphics_defaultworldmap" ) )	SetOptionValue ( "graphics_defaultworldmap", 1, "Create agents with the default world map.", true, true );

	Option *optionSoftwareRendering = GetOption ( "graphics_softwarerendering" );
	if ( !optionSoftwareRendering ) {
#ifdef WIN32
		SetOptionValue ( "graphics_softwarerendering", opengl_isSoftwareRendering (), "Enable software rendering.", true, true );
#else
		SetOptionValue ( "graphics_softwarerendering", 0, "Enable software rendering.", true, false );
#endif
	}
	else {
#ifdef WIN32
		optionSoftwareRendering->SetVisible ( true );
		optionSoftwareRendering->SetValue ( opengl_isSoftwareRendering () );
#else
		optionSoftwareRendering->SetVisible ( false );
#endif
	}

	// Sound

	if ( !GetOption ( "sound_musicenabled" ) )			SetOptionValue ( "sound_musicenabled", 1, "Enables or disables music", true, true );

	// Network

    // Theme

    // SetThemeName ( "graphics" );


    // Repair old options files

    app->GetOptions ()->GetOption ( "graphics_screenwidth" )->SetVisible ( false );
    app->GetOptions ()->GetOption ( "graphics_screenheight" )->SetVisible ( false );
    app->GetOptions ()->GetOption ( "graphics_screendepth" )->SetVisible ( false );
    app->GetOptions ()->GetOption ( "graphics_screenrefresh" )->SetVisible ( false );

}

void Options::SetThemeName ( const char *newThemeName )
{
    themeName = newThemeName;      // safe when called with GetThemeName() to re-read the theme

    //
    // Parse the theme.txt file

    char *themeTextFilename = ThemeFilename ( "theme.txt" );
    char *rsFilename = RsArchiveFileOpen ( themeTextFilename );

    if ( rsFilename ) {
        idos2unixstream thefile ( rsFilename );

        // Header

        char unused [64];
        char line [1024];
	    thefile >> unused >> ws;
        thefile.getline ( line, 128 );
        themeTitle = line;

        thefile >> unused >> ws;
        thefile.getline ( line, 128 );
        themeAuthor = line;

        thefile >> unused >> ws;
        thefile.getline ( line, 1024 );
        themeDescription = line;

        // Colours

        while ( !thefile.eof() ) {

            char lineBuffer [256];
            thefile.getline ( lineBuffer, 256 );

            if ( strlen(lineBuffer) < 2 )     continue;
		    if ( lineBuffer[0] == ';' )       continue;

            char colourName[64];
            float r, g, b;
    		std::istringstream thisLine ( lineBuffer );
            thisLine >> colourName >> ws >> r >> g >> b >> ws;

            BTree <ColourOption *> *exists = colours.LookupTree( colourName );
            if ( !exists ) {
                colours.PutData( colourName, new ColourOption ( r, g, b ) );
            }
            else {
                delete exists->data;
                exists->data = new ColourOption ( r, g, b );
            }

        }

        thefile.close();
    }


    RsArchiveFileClose ( themeTextFilename, NULL );
    delete [] themeTextFilename;

}

const char *Options::GetThemeName ()
{
    return themeName.c_str ();
}

const char *Options::GetThemeTitle ()
{
    return themeTitle.c_str ();
}

const char *Options::GetThemeDescription ()
{
    return themeDescription.c_str ();
}

ColourOption *Options::GetColour ( const char *colourName )
{

    ColourOption *result = colours.GetData (colourName);

    if ( result ) {
        return result;
    }
    else {

        printf ( "Options::GetColour WARNING : Couldn't find colour %s\n", colourName );
        return &getColourDefault;

    }

}

char *Options::ThemeFilename ( const char *filename )
{

	size_t resultsize = 256;
    char *result = new char [resultsize];

    if ( strcmp ( themeName.c_str (), "graphics" ) == 0 ) {

        UplinkSnprintf ( result, resultsize, "graphics/%s", filename );

    }
    else {

        char fullFilename[256];
        UplinkSnprintf ( fullFilename, sizeof ( fullFilename ), "%s%s/%s", app->path.c_str (), themeName.c_str (), filename );
		if ( DoesFileExist ( fullFilename ) ) {
            UplinkSnprintf ( result, resultsize, "%s/%s", themeName.c_str (), filename );

		} else {
            UplinkSnprintf ( result, resultsize, "graphics/%s", filename );
		}

    }

    return result;

}

void Options::RequestShutdownChange ( const char *optionName, int newValue )
{

    OptionChange *oc = new OptionChange ();
    oc->name = optionName;
    oc->value = newValue;

    shutdownChanges.PutData( oc );

}

void Options::ApplyShutdownChanges ()
{

    while ( shutdownChanges.GetData(0) ) {

        OptionChange *oc = shutdownChanges.GetData(0);
        shutdownChanges.RemoveData(0);

        SetOptionValue ( oc->name.c_str (), oc->value );

        delete oc;

    }

}

bool Options::Load ( [[maybe_unused]] FILE *file )
{

	// Read from our own options file

	char filename [256];
	UplinkSnprintf ( filename, sizeof ( filename ), "%soptions", app->userpath.c_str () );
	printf ( "Loading uplink options from %s...", filename );

	FILE *optionsfile = NULL;
	bool encrypted = false;
	if ( RsFileEncryptedNoVerify ( filename ) ) {
		if ( RsFileEncrypted ( filename ) ) {
			optionsfile = RsFileOpen ( filename );
			encrypted = true;
		}
		else {
			printf ( "failed\n" );
			return false;
		}
	}
	else {
		optionsfile = fopen ( filename, "rb" );
	}

	if ( optionsfile ) {

	    char version [32];
		if ( !FileReadData ( version, sizeof (SAVEFILE_VERSION), 1, optionsfile ) ) {
			version [ 0 ] = '\0';
		}
		else {
			version [ sizeof(version) - 1 ] = '\0';
		}
        if ( version[0] == '\0' || strcmp ( version, SAVEFILE_VERSION_MIN ) < 0 || strcmp ( version, SAVEFILE_VERSION ) > 0 ) {
            printf ( "\nERROR : Could not load options due to incompatible version format\n" );
			if ( encrypted )
				RsFileClose ( filename, optionsfile );
			else
				fclose ( optionsfile );
            return false;
        }

		printf ( "success\n" );

		LoadID ( optionsfile );
		if ( !LoadBTree ( (BTree <UplinkObject *> *) &options, optionsfile ) ) {
			DeleteBTreeData ( (BTree <UplinkObject *> *) &options );
			return false;
		}
		LoadID_END ( optionsfile );

		// 't' in the optionsfile at this point signals
		// theme name comes next

		char newThemeName[128];
		size_t newThemeLen;

		if ( fgetc(optionsfile) == 't' )
			if ( fread(&newThemeLen, sizeof(newThemeLen), 1, optionsfile) == 1 )
				if ( newThemeLen + 1 < 128 )
					if ( fread(newThemeName, newThemeLen, 1, optionsfile) == 1 ) {
						  newThemeName[newThemeLen] = '\0';
						  themeName = newThemeName;
					}

		if ( encrypted )
			RsFileClose ( filename, optionsfile );
		else
			fclose ( optionsfile );

	}
	else {

//        float currentVersion;
//        sscanf ( VERSION_NUMBER, "%f", &currentVersion );
//        currentVersion *= 100;
//        SetOptionValue ( "game_version", currentVersion, "z", false, false );

		printf ( "failed\n" );
		return false;

	}

	return true;

}

void Options::Save ( [[maybe_unused]] FILE *file )
{

	// Write to our own options file

	MakeDirectory ( app->userpath.c_str () );

	char filename [256];
	UplinkSnprintf ( filename, sizeof ( filename ), "%soptions", app->userpath.c_str () );

	printf ( "Saving uplink options to %s...", filename );

	FILE *optionsfile = fopen ( filename, "wb" );

	if ( optionsfile ) {

		printf ( "success\n" );

		fwrite ( SAVEFILE_VERSION, sizeof (SAVEFILE_VERSION), 1, optionsfile );

		SaveID ( optionsfile );
		SaveBTree ( (BTree <UplinkObject *> *) &options, optionsfile );
		SaveID_END ( optionsfile );

		fputc('t', optionsfile);
		size_t themeLen = strlen(themeName.c_str ());
		fwrite(&themeLen, sizeof(themeLen), 1, optionsfile);
		fwrite(themeName.c_str (), themeLen, 1, optionsfile);

		fclose ( optionsfile );

		RsEncryptFile ( filename );

	}
	else {

		printf ( "failed\n" );

	}

}

void Options::Print ()
{

	printf ( "============== O P T I O N S ===============================\n" );

	PrintBTree ( (BTree <UplinkObject *> *) &options );

	printf ( "============================================================\n" );

}

void Options::Update ()
{
}

const char *Options::GetID ()
{

	return "OPTIONS";

}

Options *options;


// ============================================================================
// Option class


Option::Option()
{

	name = "";
	tooltip = "";
	yesorno = false;
	visible = true;
	value = 0;

}

Option::~Option()
{

}

void Option::SetName ( const char *newname )
{

	UplinkAssert ( strlen(newname) < SIZE_OPTION_NAME );
	name = newname;

}

void Option::SetTooltip ( const char *newtooltip )
{

	UplinkAssert ( strlen(newtooltip) < SIZE_OPTION_TOOLTIP );
	tooltip = newtooltip;

}

void Option::SetYesOrNo	( bool newyesorno )
{

	yesorno = newyesorno;

}

void Option::SetVisible ( bool newvisible )
{

	visible = newvisible;

}

void Option::SetValue ( int newvalue )
{

	value = newvalue;

}

bool Option::Load ( FILE *file )
{

	LoadID ( file );

	if ( !LoadFixedString ( name, SIZE_OPTION_NAME, file ) ) return false;

	if ( !LoadFixedString ( tooltip, SIZE_OPTION_TOOLTIP, file ) ) return false;

	if ( !FileReadData ( &yesorno, sizeof(yesorno), 1, file ) ) return false;
	if ( !FileReadData ( &visible, sizeof(visible), 1, file ) ) return false;
	if ( !FileReadData ( &value, sizeof(value), 1, file ) ) return false;

	LoadID_END ( file );

	return true;

}

void Option::Save ( FILE *file )
{

	SaveID ( file );

	SaveFixedString ( name, SIZE_OPTION_NAME, file );
	SaveFixedString ( tooltip, SIZE_OPTION_TOOLTIP, file );
	fwrite ( &yesorno, sizeof(yesorno), 1, file );
	fwrite ( &visible, sizeof(visible), 1, file );
	fwrite ( &value, sizeof(value), 1, file );

	SaveID_END ( file );

}

void Option::Print ()
{

	printf ( "Option : name=%s, value=%d\n", name.c_str (), value );
	printf ( "\tYesOrNo=%d, Visible=%d\n", yesorno, visible );

}

void Option::Update ()
{
}

const char *Option::GetID ()
{

	return "OPTION";

}

int Option::GetOBJECTID ()
{

	return OID_OPTION;

}
