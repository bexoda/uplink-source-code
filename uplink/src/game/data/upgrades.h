// The software and hardware sold in the game, read from data/upgrades.tsv.
//
// One row per version, in the order the shops list them. Columns, tab separated:
//   kind (software | hardware), name, type, cost, size, data, description
// "data" is the software version, or the hardware's speed / capacity / bandwidth.
// In the description, \n is a line break and \\ a backslash. Lines starting with # are
// comments. The file is looked up like any game file (next to the game, then in archives).

#ifndef _included_upgrades_h
#define _included_upgrades_h

#include <string>
#include <vector>

#include "game/data/data.h"

const std::vector <ComputerUpgrade> &SoftwareUpgrades ();		// Loads the file on first use
const std::vector <ComputerUpgrade> &HardwareUpgrades ();

const ComputerUpgrade *GetSoftwareUpgrade ( const char *name );	// First row with this name, or NULL
const ComputerUpgrade *GetHardwareUpgrade ( const char *name );

// Replaces both tables with the contents of a file. False (and why) if it cannot be read.
bool LoadUpgrades ( const char *path, std::string *error );

// The tables in the file's format: LoadUpgrades of this text gives the same tables.
std::string UpgradesToText ();

#endif
