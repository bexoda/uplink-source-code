// Fatal errors, assertions and bounded string helpers declared in app/globals.h.

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef WIN32
#include <windows.h>
#endif

#include "app/globals.h"
#include "uplink.h"

[[noreturn]] void UplinkFatal ()
{
#if defined(WIN32)
	// The Windows crash reporter is structured exception handling around WinMain.
	RaiseException ( EXCEPTION_ACCESS_VIOLATION, EXCEPTION_NONCONTINUABLE, 0, NULL );
#elif defined(TESTGAME)
	GciRestoreScreenSize ();
	abort ();
#else
	RunUplinkExceptionHandling ();	// reports, restores the screen mode and exits
#endif
	abort ();
}

[[noreturn]] void UplinkAssertFailed ( const char *condition, const char *file, int line )
{
	printf ( "\n"
	         "An Uplink Assertion Failure has occured\n"
	         "=======================================\n"
	         " Condition : %s\n"
	         " Location  : %s, line %d\n",
	         condition, file, line );
	UplinkFatal ();
}

[[noreturn]] void UplinkAbortImpl ( const char *file, int line, const char *msg )
{
	printf ( "\n"
	         "Uplink has been forced to Abort\n"
	         "===============================\n"
	         " Message   : %s\n"
	         " Location  : %s, line %d\n",
	         msg, file, line );
	UplinkFatal ();
}

[[noreturn]] void UplinkAbortArgsImpl ( const char *file, int line, const char *format, ... )
{
	printf ( "\n"
	         "Uplink has been forced to Abort\n"
	         "===============================\n"
	         " Message   : " );
	va_list args;
	va_start ( args, format );
	vprintf ( format, args );
	va_end ( args );
	printf ( "\n"
	         " Location  : %s, line %d\n",
	         file, line );
	UplinkFatal ();
}

void UplinkSnprintfImpl ( const char *file, int line, char *buf, size_t bufsize, const char *format, ... )
{
	if ( bufsize == 0 ) return;

	va_list args;
	va_start ( args, format );
	int written = vsnprintf ( buf, bufsize, format, args );
	va_end ( args );
	buf [ bufsize - 1 ] = '\0';

	if ( written < 0 || (size_t) written >= bufsize ) {
		printf ( "\n"
		         "An Uplink snprintf Failure has occured\n"
		         "======================================\n"
		         " Location    : %s, line %d\n"
		         " Buffer size : %zu\n"
		         " Format      : %s\n"
		         " Buffer      : %s\n",
		         file, line, bufsize, format, buf );
		UplinkFatal ();
	}
}

void UplinkStrncpyImpl ( const char *file, int line, char *dest, const char *source, size_t count )
{
	if ( count == 0 ) return;

	size_t sourcelen = strlen ( source );
	if ( sourcelen >= count ) {
		printf ( "\n"
		         "An Uplink strncpy Failure has occured\n"
		         "=====================================\n"
		         " Location    : %s, line %d\n"
		         " Dest. size  : %zu\n"
		         " Source size : %zu\n"
		         " Str. Source : %s\n",
		         file, line, count, sourcelen, source );
		UplinkFatal ();
	}
	// Like strncpy, zero the rest of the buffer: whole char arrays are written to save files.
	// memmove because callers sometimes pass the destination as the source.
	memmove ( dest, source, sourcelen );
	memset ( dest + sourcelen, 0, count - sourcelen );
}

void UplinkStrncatImpl ( const char *file, int line, char *dest, size_t destbufsize, const char *source )
{
	if ( destbufsize == 0 ) return;

	size_t destlen = strlen ( dest );
	size_t sourcelen = strlen ( source );
	if ( destlen >= destbufsize || sourcelen > ( destbufsize - 1 ) - destlen ) {
		printf ( "\n"
		         "An Uplink strncat Failure has occured\n"
		         "=====================================\n"
		         " Location         : %s, line %d\n"
		         " Dest. buf. size  : %zu\n"
		         " Dest. size       : %zu\n"
		         " Source size      : %zu\n"
		         " Str. Dest.       : %s\n"
		         " Str. Source      : %s\n",
		         file, line, destbufsize, destlen, sourcelen, dest, source );
		UplinkFatal ();
	}
	memcpy ( dest + destlen, source, sourcelen + 1 );
}

char *UplinkStrdup ( const char *s )
{
	size_t len = strlen ( s );
	char *copy = new char [ len + 1 ];
	memcpy ( copy, s, len + 1 );
	return copy;
}

std::string UplinkFormat ( const char *format, ... )
{
	va_list args;
	va_start ( args, format );
	va_list copy;
	va_copy ( copy, args );
	int len = vsnprintf ( NULL, 0, format, copy );
	va_end ( copy );
	std::string out;
	if ( len > 0 ) {
		out.resize ( (size_t) len );
		vsnprintf ( &out [0], (size_t) len + 1, format, args );
	}
	va_end ( args );
	return out;
}
