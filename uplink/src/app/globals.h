/*

	ALL global objects / constants are defined here

  */

#ifndef _included_globals_h
#define _included_globals_h

#include "globals_defines.h"

// ===========================================================================
// My very own, special assertion function
//

#include "gucci.h"


#ifdef WIN32
#define UplinkIntFlush _flushall()
#else
#define UplinkIntFlush fflush(NULL)
#endif


// Fatal errors, assertions and bounded string helpers.
//
// These used to be macros that crashed on purpose by writing through a NULL pointer, so the
// SIGSEGV handler would report the error. They now call UplinkFatal (app/globals.cpp), which
// runs the same crash report directly. The macros remain only to capture __FILE__/__LINE__.

#if defined(__GNUC__)
#define UPLINK_PRINTF(fmt,args) __attribute__ (( format ( printf, fmt, args ) ))
#else
#define UPLINK_PRINTF(fmt,args)
#endif

#include <stddef.h>
#include <string>

[[noreturn]] void UplinkFatal ();
[[noreturn]] void UplinkAssertFailed ( const char *condition, const char *file, int line );
[[noreturn]] void UplinkAbortImpl ( const char *file, int line, const char *msg );
[[noreturn]] void UplinkAbortArgsImpl ( const char *file, int line, const char *format, ... ) UPLINK_PRINTF ( 3, 4 );

void UplinkSnprintfImpl ( const char *file, int line, char *buf, size_t bufsize, const char *format, ... ) UPLINK_PRINTF ( 5, 6 );
void UplinkStrncpyImpl ( const char *file, int line, char *dest, const char *source, size_t count );
void UplinkStrncatImpl ( const char *file, int line, char *dest, size_t destbufsize, const char *source );

// printf into a std::string (no length limit).
std::string UplinkFormat ( const char *format, ... ) UPLINK_PRINTF ( 1, 2 );

// A new[] copy of `s`, for functions whose callers delete [] the result.
char *UplinkStrdup ( const char *s );

#define UplinkAssert(x)  { if ( !(x) ) UplinkAssertFailed ( #x, __FILE__, __LINE__ ); }

#ifdef TESTGAME
	#define UplinkPrintAssert(x) UplinkAssert(x)
#else
	#define UplinkPrintAssert(x) {											\
	    if (!(x)) {															\
	        printf ( "Print Assert: %s ln %d : %s\n",						\
	                 __FILE__, __LINE__, #x );								\
	    }																	\
	}
#endif

#define UplinkAbort(msg)          UplinkAbortImpl ( __FILE__, __LINE__, msg )
#define UplinkAbortArgs(msg,...)  UplinkAbortArgsImpl ( __FILE__, __LINE__, msg, __VA_ARGS__ )

#ifdef TESTGAME
	#define UplinkPrintAbort(msg)         UplinkAbort(msg)
	#define UplinkPrintAbortArgs(msg,...) UplinkAbortArgs(msg,__VA_ARGS__)
#else
	#define UplinkPrintAbort(msg) {											\
	    printf ( "Print Abort: %s ln %d :\n%s\n",							\
			     __FILE__, __LINE__, msg );									\
	}
	#define UplinkPrintAbortArgs(msg,...) {									\
	    printf ( "Print Abort: %s ln %d : ",								\
			     __FILE__, __LINE__ );										\
		printf ( msg, __VA_ARGS__ );										\
		printf ( "\n" );													\
	}
#endif

//
// Warning - print a message (when in DEBUG mode)
//

#ifdef _DEBUG
#define  UplinkWarning(msg) {											\
  printf ( "WARNING: %s ln %d :\n%s\n",									\
			__FILE__, __LINE__, msg );									\
}
#else
#define	 UplinkWarning(msg) ((void) 0)
#endif

//
// Our very own, special snprintf, strncpy, strncat functions.
// A result that does not fit its buffer is a fatal error, as it always was.
//

#ifdef WIN32
#define UplinkIntSnprintf _snprintf
#else
#define UplinkIntSnprintf snprintf
#endif

#define UplinkSnprintf(buf,bufsize,...)                UplinkSnprintfImpl ( __FILE__, __LINE__, buf, bufsize, __VA_ARGS__ )
#define UplinkStrncpy(strDest,strSource,count)         UplinkStrncpyImpl ( __FILE__, __LINE__, strDest, strSource, count )
#define UplinkStrncat(strDest,strDestBufSize,strSource) UplinkStrncatImpl ( __FILE__, __LINE__, strDest, strDestBufSize, strSource )

#define UplinkSafeStrcpy(strDest,strSource) {														\
			strcpy ( strDest, strSource );															\
    }

#define UplinkSafeStrcat(strDest,strSource) {														\
			strcat ( strDest, strSource );															\
    }

#define UplinkMin(a,b) (((a) < (b)) ? (a) : (b))


#endif  // _included_globals_h
