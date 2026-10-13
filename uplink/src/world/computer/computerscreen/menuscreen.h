
/*

  MenuScreen class object

  Represents a computer menu screen, with a list of possible options
  and the action to take when those options are selected

  */


#ifndef _included_menuscreen_h
#define _included_menuscreen_h

#include <string>

// ============================================================================

#include "app/uplinkobject.h"

#include "world/computer/computerscreen/computerscreen.h"


class MenuScreenOption;

// ============================================================================

class MenuScreen : public ComputerScreen  
{

protected:

	LList <MenuScreenOption *> options;	

public:

	MenuScreen();
	virtual ~MenuScreen();

	void  AddOption	  ( const char *caption, const char *tooltip, int nextpage, int security = 10, int index = -1 );
	int   NumOptions  ();
	const char *GetCaption  ( int index );
	const char *GetTooltip  ( int index );
	int   GetNextPage ( int index );
	int   GetSecurity ( int index );
 

	// Common functions

	bool Load  ( FILE *file );			
	void Save  ( FILE *file );			
	void Print ();						
	
	const char *GetID ();						
	int   GetOBJECTID ();				

};


// ============================================================================

#define SIZE_MENUSCREENOPTION_CAPTION   64
#define SIZE_MENUSCREENOPTION_TOOLTIP   128


class MenuScreenOption : public UplinkObject
{

public :

	std::string caption;
	std::string tooltip;
	int nextpage;
	int security;

public:

	MenuScreenOption ();
	~MenuScreenOption ();

	void SetCaption ( const char *newcaption );
	void SetTooltip ( const char *newtooltip );
	void SetNextPage ( int newnextpage );
	void SetSecurity ( int newsecurity );


	// Common functions

	bool Load  ( FILE *file );
	void Save  ( FILE *file );
	void Print ();
	void Update ();

	const char *GetID ();
	int GetOBJECTID ();
	
};


#endif
