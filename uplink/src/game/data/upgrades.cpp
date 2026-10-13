#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <fstream>
#include <sstream>

#include "redshirt.h"

#include "app/globals.h"
#include "game/data/upgrades.h"

namespace {

std::vector <ComputerUpgrade> software;
std::vector <ComputerUpgrade> hardware;
std::deque <std::string> strings;			// ComputerUpgrade points into these; a deque never moves them
bool loaded = false;

const char *Keep ( const std::string &s )
{
	strings.push_back ( s );
	return strings.back ().c_str ();
}

std::string Escape ( const char *s )
{
	std::string out;
	for ( ; *s; ++s ) {
		if ( *s == '\\' ) out += "\\\\";
		else if ( *s == '\n' ) out += "\\n";
		else out += *s;
	}
	return out;
}

bool Unescape ( const std::string &s, std::string &out )
{
	out.clear ();
	for ( size_t i = 0; i < s.size (); ++i ) {
		if ( s [i] != '\\' ) { out += s [i]; continue; }
		if ( ++i == s.size () ) return false;
		if ( s [i] == 'n' ) out += '\n';
		else if ( s [i] == '\\' ) out += '\\';
		else return false;
	}
	return true;
}

bool ToInt ( const std::string &s, int &value )
{
	if ( s.empty () ) return false;
	char *end = nullptr;
	long v = strtol ( s.c_str (), &end, 10 );
	if ( *end ) return false;
	value = (int) v;
	return true;
}

void LoadOnce ()
{
	if ( loaded ) return;
	char *path = RsArchiveFileOpen ( "data/upgrades.tsv" );
	std::string error;
	if ( !path || !LoadUpgrades ( path, &error ) ) {
		if ( !path ) error = "data/upgrades.tsv not found";
		UplinkAbortArgs ( "Cannot load the software and hardware tables: %s", error.c_str () );
	}
	RsArchiveFileClose ( "data/upgrades.tsv" );
}

}

bool LoadUpgrades ( const char *path, std::string *error )
{
	std::ifstream in ( path );
	if ( !in ) { if ( error ) *error = std::string ( "cannot open " ) + path; return false; }

	std::vector <ComputerUpgrade> sw, hw;
	std::string line;
	int lineno = 0;
	while ( std::getline ( in, line ) ) {
		++lineno;
		if ( !line.empty () && line.back () == '\r' ) line.pop_back ();
		if ( line.empty () || line [0] == '#' ) continue;

		std::vector <std::string> f;
		std::stringstream ss ( line );
		std::string field;
		while ( std::getline ( ss, field, '\t' ) ) f.push_back ( field );

		ComputerUpgrade cu;
		std::string description;
		if ( f.size () != 7 || ( f [0] != "software" && f [0] != "hardware" ) ||
		     !ToInt ( f [2], cu.TYPE ) || !ToInt ( f [3], cu.cost ) ||
		     !ToInt ( f [4], cu.size ) || !ToInt ( f [5], cu.data ) || !Unescape ( f [6], description ) ) {
			if ( error ) *error = std::string ( path ) + ":" + std::to_string ( lineno ) + ": not a valid row";
			return false;
		}
		cu.name = Keep ( f [1] );
		cu.description = Keep ( description );
		( f [0] == "software" ? sw : hw ).push_back ( cu );
	}

	software = std::move ( sw );
	hardware = std::move ( hw );
	loaded = true;
	return true;
}

const std::vector <ComputerUpgrade> &SoftwareUpgrades ()
{
	LoadOnce ();
	return software;
}

const std::vector <ComputerUpgrade> &HardwareUpgrades ()
{
	LoadOnce ();
	return hardware;
}

static const ComputerUpgrade *Find ( const std::vector <ComputerUpgrade> &table, const char *name )
{
	for ( const ComputerUpgrade &cu : table )
		if ( strcmp ( cu.name, name ) == 0 )
			return &cu;
	return NULL;
}

const ComputerUpgrade *GetSoftwareUpgrade ( const char *name )
{
	return Find ( SoftwareUpgrades (), name );
}

const ComputerUpgrade *GetHardwareUpgrade ( const char *name )
{
	return Find ( HardwareUpgrades (), name );
}

std::string UpgradesToText ()
{
	std::string out =
		"# Software and hardware sold in Uplink, one row per version, in shop order.\n"
		"# kind\tname\ttype\tcost\tsize\tdata\tdescription\n"
		"# data: software version, or hardware speed / capacity / bandwidth.\n"
		"# description: \\n is a line break, \\\\ a backslash.\n"
		"# type (software): 1 file utility, 2 hardware driver, 3 security, 4 cracker, 5 bypasser,\n"
		"#   6 LAN tool, 9 HUD upgrade, 10 other.\n";
	for ( const auto *table : { &SoftwareUpgrades (), &HardwareUpgrades () } ) {
		for ( const ComputerUpgrade &cu : *table ) {
			out += ( table == &software ) ? "software" : "hardware";
			out += "\t"; out += cu.name;
			out += "\t" + std::to_string ( cu.TYPE ) + "\t" + std::to_string ( cu.cost );
			out += "\t" + std::to_string ( cu.size ) + "\t" + std::to_string ( cu.data );
			out += "\t" + Escape ( cu.description ) + "\n";
		}
	}
	return out;
}
