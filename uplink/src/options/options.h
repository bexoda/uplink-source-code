// -*- tab-width:4; c-file-mode:"cc-mode" -*-

/*

  Options class object

	Part of the GAME subsystem
	Stores all of the options specified by the user.

	This class is different in that the Load/Save methods don't write to the 
	specified file - they write to their own options file.

  */

#ifndef _included_options_h
#define _included_options_h

#include <string>

#include "app/globals.h"
#include "app/uplinkobject.h"


class Option;
class OptionChange;
class ColourOption;


class Options : public UplinkObject  
{

protected:

	BTree <Option *> options;
    LList <OptionChange *> shutdownChanges;

    std::string themeName;
    std::string themeAuthor;
    std::string themeTitle;
    std::string themeDescription;
    BTree <ColourOption *> colours;

public:

	Options();
	virtual ~Options();

	void CreateDefaultOptions ();													// Fills in all options not yet created


	Option *GetOption ( const char *name );												// Returns NULL if not found
	int     GetOptionValue ( const char *name );											// asserts existence of name
	bool    IsOptionEqualTo ( const char *name, int value );
	
	void    SetOptionValue ( const char *name, int newvalue );							// Does not create new 
	void	SetOptionValue ( const char *name, int newvalue, const char *tooltip, 
							 bool yesorno = false, bool visible = true );			// Creates new if neccisary

	LList <Option *> *GetAllOptions ( const char *searchstring, bool returnhidden );		// String can be NULL - will return ALL

    void SetThemeName ( const char *newThemeName );
    const char *GetThemeName ();
    const char *GetThemeTitle ();
    const char *GetThemeDescription ();
    
    ColourOption *GetColour     ( const char *colourName );                                   // Always safe - returns BLACK if not found

    char *ThemeFilename ( const char *filename );

    void RequestShutdownChange ( const char *optionName, int newValue );
    void ApplyShutdownChanges ();

	// Common functions

	bool Load   ( FILE *file );														// Does not actually save to file
	void Save   ( FILE *file );														// "" ""
	void Print  ();
	void Update ();
	const char *GetID ();

};

// ============================================================================


#define SIZE_OPTION_NAME		64
#define SIZE_OPTION_TOOLTIP		128


class Option : public UplinkObject
{

public:

	std::string name;
	std::string tooltip;

	bool yesorno;										// Is this a boolean option
	bool visible;										// Is this a hidden option
	int value;

public:

	Option();
	virtual ~Option();

	void SetName    ( const char *newname );
	void SetTooltip ( const char *newtooltip );
	void SetYesOrNo	( bool newyesorno );
	void SetVisible ( bool newvisible );
	void SetValue   ( int newvalue );


	// Common functions

	bool Load   ( FILE *file );
	void Save   ( FILE *file );
	void Print  ();
	void Update ();
	const char *GetID ();
	int  GetOBJECTID ();

};


class OptionChange
{
public:
    OptionChange () {};

    std::string name;
    int value;
};

class ColourOption
{

public:

    ColourOption ( float _r, float _g, float _b )
        : r ( _r ), g ( _g ), b ( _b ) {}

    float r;
    float g;
    float b;

};

#endif 
