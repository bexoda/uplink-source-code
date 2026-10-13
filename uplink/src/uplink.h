/*
	Start-up and shutdown of the whole game.

	main() only installs crash handlers and calls RunUplink. The Init_* steps are
	public so tests can bring up the parts they need (data, options, game) without
	a window or sound.
*/

#ifndef _included_uplink_h
#define _included_uplink_h

void RunUplink ( int argc, char **argv );
int RunUplinkExceptionHandling ();

void Init_App      ( char *apppath );
void Init_Options  ( int argc, char **argv );

bool Load_Data	   ();
void Init_Game     ();
void Init_Sound    ();
void Init_Music    ();
void Init_Graphics ();
void Init_OpenGL   ( int argc, char **argv );
void Init_Fonts	   ();

void Run_MainMenu  ();
void Run_Game      ();

void Cleanup_Uplink();

#if defined(FULLGAME) || defined(TESTGAME)
#if defined(WIN32)
bool Init_Steam    ();
bool Cleanup_Steam ();
#endif

bool VerifyLegitAndCodeCardCheck();
#endif

#ifndef WIN32
void hSignalSIGSEGV ( int n );
void hSignalSIGFPE  ( int n );
void hSignalSIGPIPE ( int n );
#endif

#endif
