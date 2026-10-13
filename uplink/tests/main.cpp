// Test entry point. The game redirects stdout and stderr into ~/.uplink/debug.log when it
// boots, which would swallow the test report too. Keep a handle on the real stdout and point
// Catch2's reporter at it; the game's own output stays in debug.log.

#include <catch2/catch_session.hpp>

#include <string>
#include <vector>
#include <unistd.h>

int main ( int argc, char **argv )
{
	std::vector<std::string> args ( argv, argv + argc );

	bool hasOut = false;
	for ( auto &a : args )
		if ( a == "-o" || a == "--out" || a.rfind ( "--out=", 0 ) == 0 ) hasOut = true;

	if ( !hasOut ) {
		int fd = dup ( STDOUT_FILENO );
		if ( fd != -1 ) {
			args.push_back ( "--out" );
			args.push_back ( "/dev/fd/" + std::to_string ( fd ) );
		}
	}

	std::vector<char *> cargs;
	for ( auto &a : args ) cargs.push_back ( a.data () );
	return Catch::Session ().run ( (int) cargs.size (), cargs.data () );
}
