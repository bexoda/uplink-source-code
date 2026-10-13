
#ifndef _included_dialler_h
#define _included_dialler_h


#include "interface/localinterface/localinterfacescreen.h"


class PhoneDialler : public LocalInterfaceScreen
{

protected:

	static char buttonnames [][9];

	char *ip;
	char *infosup;

	int nextscene;

	int lastupdate;
	int curnumber;

	void UpdateDisplay ();

public:

    PhoneDialler ();
    ~PhoneDialler ();

	using LocalInterfaceScreen::Create;			// adds a positioned Create, keeps the base one
	void Create ( int x, int y );
	void Remove ();
	bool UpdateSpecial ();
	bool IsVisible ();

	void DialNumber ( int x, int y, const char *number, int scenario, char *info = NULL );

};

#endif

