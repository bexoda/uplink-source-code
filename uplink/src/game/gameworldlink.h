// GameWorldLink: WorldLink for the running game. Each call does exactly what the world
// code used to do itself through the interface, Eclipse and Soundgarden.

#ifndef _included_gameworldlink_h
#define _included_gameworldlink_h

#include "world/worldlink.h"

class GameWorldLink : public WorldLink
{

public:

	double TimeMs () override;

	void ShowDisconnected () override;
	void GameSpeedChanged () override;
	void PlaySound ( const char *filename, bool synchronised ) override;
	void CriticalError ( const char *message ) override;
	void SetLanAlarm ( bool on ) override;
	void SetRemoteSecurity ( const char *name, int level ) override;
	void PlayerLinksChanged ( LList <char *> *links ) override;
	void DialogWidgetRemoved ( DialogScreen *screen, DialogScreenWidget *widget ) override;

	void RunPlayerSoftware ( const char *name, float version ) override;

	Computer *ScreenComputer () override;
	const char *ScreenSecurityName () override;

};

#endif
