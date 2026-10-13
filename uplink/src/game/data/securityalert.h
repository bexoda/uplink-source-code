// Alert levels: how a computer's defences tighten after break-ins it has noticed.
// Read from data/security.tsv (format at the top of that file).

#ifndef _included_securityalert_h
#define _included_securityalert_h

#include <string>
#include <vector>

#define MAX_SECURITY_LEVEL 5							// The highest version of the tools that test a level

struct SecurityAlert
{
	int level;										// 0 = calm, rising
	std::string name;
	int recenthacks;									// Applies from this many recent hacks
	float trace;										// Multiplies the time per trace hop
	int securitybonus;									// Added to security system levels
	bool newcodesonconnect;							// Passwords change when the player connects
};

const std::vector <SecurityAlert> &SecurityAlerts ();		// Loads the file on first use, sorted by level
const SecurityAlert &SecurityAlertFor ( int recenthacks );

bool LoadSecurityAlerts ( const char *path, std::string *error );

#endif
