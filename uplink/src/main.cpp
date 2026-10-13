// Program entry point on Linux and macOS. Windows enters through WinMain in uplink.cpp.

#include <signal.h>

#include "uplink.h"

int main ( int argc, char **argv )
{

#if !defined(WIN32) && !defined(_DEBUG)
	signal ( SIGSEGV, hSignalSIGSEGV );  // segmentation violation
	signal ( SIGFPE, hSignalSIGFPE );    // floating point exception: "erroneous arithmetic operation"
	signal ( SIGPIPE, hSignalSIGPIPE );  // write to pipe with no one reading
#endif

	RunUplink ( argc, argv );
	return 0;
}
