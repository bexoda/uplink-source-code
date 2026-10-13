#include <cstdlib>
#include <fstream>
#include <sstream>

#include "redshirt.h"

#include "app/globals.h"
#include "game/data/securityalert.h"

namespace {

std::vector <SecurityAlert> alerts;
bool loaded = false;

void LoadOnce ()
{
	if ( loaded ) return;
	char *path = RsArchiveFileOpen ( "data/security.tsv" );
	std::string error;
	if ( !path || !LoadSecurityAlerts ( path, &error ) ) {
		if ( !path ) error = "data/security.tsv not found";
		UplinkAbortArgs ( "Cannot load the security alert table: %s", error.c_str () );
	}
	RsArchiveFileClose ( "data/security.tsv" );
}

}

bool LoadSecurityAlerts ( const char *path, std::string *error )
{
	std::ifstream in ( path );
	if ( !in ) { if ( error ) *error = std::string ( "cannot open " ) + path; return false; }

	std::vector <SecurityAlert> rows;
	std::string line;
	int lineno = 0;
	while ( std::getline ( in, line ) ) {
		++lineno;
		if ( !line.empty () && line.back () == '\r' ) line.pop_back ();
		if ( line.empty () || line [0] == '#' ) continue;

		std::stringstream ss ( line );
		SecurityAlert a;
		int newcodes = -1;
		std::string rest;
		bool ok = (bool) ( ss >> a.level >> a.name >> a.recenthacks >> a.trace >> a.securitybonus >> newcodes );
		if ( !ok || ( ss >> rest ) || a.level != (int) rows.size () || ( newcodes != 0 && newcodes != 1 ) ||
		     a.trace <= 0.0f || a.securitybonus < 0 ||
		     ( !rows.empty () && a.recenthacks <= rows.back ().recenthacks ) ) {
			if ( error ) *error = std::string ( path ) + ":" + std::to_string ( lineno ) + ": not a valid row";
			return false;
		}
		a.newcodesonconnect = newcodes == 1;
		rows.push_back ( a );
	}
	if ( rows.empty () || rows [0].recenthacks != 0 ) {
		if ( error ) *error = std::string ( path ) + ": the first row must be level 0 at 0 recent hacks";
		return false;
	}

	alerts = std::move ( rows );
	loaded = true;
	return true;
}

const std::vector <SecurityAlert> &SecurityAlerts ()
{
	LoadOnce ();
	return alerts;
}

const SecurityAlert &SecurityAlertFor ( int recenthacks )
{
	const std::vector <SecurityAlert> &table = SecurityAlerts ();
	size_t i = 0;
	while ( i + 1 < table.size () && recenthacks >= table [i + 1].recenthacks ) ++i;
	return table [i];
}
