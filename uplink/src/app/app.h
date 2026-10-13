// -*- tab-width:4 c-file-style:"cc-mode" -*-

/*
	App object

	Stores data about the app - eg path, version, etc

  */

#ifndef _included_app_h
#define _included_app_h

#include <string>

// ============================================================================

#include "app/uplinkobject.h"


class Options;
class Network;
class MainMenu;
class PhoneDialler;


#define SIZE_APP_PATH    256
#define SIZE_APP_VERSION 32
#define SIZE_APP_TYPE    32
#define SIZE_APP_DATE    32
#define SIZE_APP_TITLE   64
#define SIZE_APP_RELEASE 256

// ============================================================================


class App : public UplinkObject
{

public :

    std::string path;
    std::string userpath;
    std::string usertmppath; // Used by the crash reporter to store the current .usr
    std::string userretirepath; // Used to store old agents ( .usr / .tmp )
    std::string version;
    std::string type;
    std::string date;
    std::string title;
    std::string release;          

    int starttime;
    bool closed;

    Options *options;
	Network *network;
	MainMenu *mainmenu;
	PhoneDialler *phoneDial;

	char *nextLoadGame;

	bool askCodeCard;

public:

    App ();
    ~App ();
    
    void Set ( const char *newpath, const char *newversion, const char *newtype,
			   const char *newdate, const char *newtitle );
    
	void Initialise ();	
	void Close ();			// Shuts down the app	
    bool Closed ();         // True if in the process of shutting down

	void SetNextLoadGame ( const char *username );       // Set the username to load with the next call to LoadGame
	void LoadGame ();                                    // Use the username set with SetNextLoadGame
	void LoadGame ( char *username );
	void SaveGame ( const char *username );
	void RetireGame ( char *username );
	static DArray <char *> *ListExistingGames ();

	Options *GetOptions ();
	Network *GetNetwork ();
	MainMenu *GetMainMenu ();
	void RegisterPhoneDialler ( PhoneDialler *phoneDiallerScreen );
	void UnRegisterPhoneDialler ( PhoneDialler *phoneDiallerScreen );

    static void CoreDump ();             

	// Common functions

	bool Load   ( FILE *file );
	void Save   ( FILE *file );
	void Print  ();
	void Update ();
	const char *GetID ();

};

extern App *app;

#endif


