// WorldLink: the simulation's only way to reach the player's screen, speakers and real clock.
//
// Code under world/ calls GetWorldLink () instead of the interface, Eclipse or Soundgarden.
// The game installs GameWorldLink (game/gameworldlink.cpp), which drives the real interface.
// With nothing installed the world runs headless: nothing is shown or played, queries about
// the screen answer "nothing", and the clock is the system's.

#ifndef _included_worldlink_h
#define _included_worldlink_h

template <class T> class LList;
class Computer;
class DialogScreen;
class DialogScreenWidget;

class WorldLink
{

public:

	virtual ~WorldLink () = default;

	// Clock
	virtual double TimeMs ();										// Real time, milliseconds

	// What the player sees and hears
	virtual void ShowDisconnected () {}							// Show the remote screen for a lost connection
	virtual void GameSpeedChanged () {}
	virtual void PlaySound ( const char * /*filename*/, bool /*synchronised*/ = true ) {}
	virtual void CriticalError ( const char * /*message*/ ) {}
	virtual void SetLanAlarm ( bool /*on*/ ) {}					// The HUD's LAN button flashes, a siren sounds
	virtual void SetRemoteSecurity ( const char * /*name*/, int /*level*/ ) {}
	virtual void PlayerLinksChanged ( LList <char *> * /*links*/ ) {}
	virtual void DialogWidgetRemoved ( DialogScreen * /*screen*/, DialogScreenWidget * /*widget*/ ) {}

	// Software that runs on the player's gateway
	virtual void RunPlayerSoftware ( const char * /*name*/, float /*version*/ ) {}

	// What the player is looking at
	virtual Computer *ScreenComputer () { return nullptr; }		// The computer on the remote screen
	virtual const char *ScreenSecurityName () { return ""; }		// Who the player is logged in as there

};

WorldLink &GetWorldLink ();
void SetWorldLink ( WorldLink *link );							// NULL: back to headless

#endif
