// OptString replaces nullable char * members; its save format must match what NULL and
// non-NULL char * strings were saved as.

#include <catch2/catch_test_macros.hpp>

#include <cstdio>
#include <cstring>
#include <vector>

#include "app/optstring.h"
#include "app/serialise.h"

static std::vector<unsigned char> Saved ( void (*save) ( FILE * ) )
{
	FILE *f = tmpfile ();
	save ( f );
	std::vector<unsigned char> bytes ( ftell ( f ) );
	rewind ( f );
	if ( !bytes.empty () ) REQUIRE ( fread ( bytes.data (), bytes.size (), 1, f ) == 1 );
	fclose ( f );
	return bytes;
}

TEST_CASE ( "OptString: unset behaves like a NULL char *", "[optstring]" )
{
	OptString s;
	CHECK_FALSE ( s );
	CHECK ( s.c_str () == NULL );

	s = "abc";
	CHECK ( s );
	CHECK ( strcmp ( s.c_str (), "abc" ) == 0 );

	s = (const char *) NULL;
	CHECK_FALSE ( s );
	CHECK ( s.c_str () == NULL );

	s = "";
	CHECK ( s );						// empty is not unset
	CHECK ( strcmp ( s.c_str (), "" ) == 0 );

	s = s.c_str ();						// self-assignment through c_str
	CHECK ( s );
}

TEST_CASE ( "OptString saves the same bytes as char *", "[optstring][save]" )
{
	CHECK ( Saved ( [] ( FILE *f ) { SaveDynamicString ( OptString (), f ); } ) ==
	        Saved ( [] ( FILE *f ) { SaveDynamicString ( (const char *) NULL, f ); } ) );
	CHECK ( Saved ( [] ( FILE *f ) { SaveDynamicString ( OptString ( "" ), f ); } ) ==
	        Saved ( [] ( FILE *f ) { SaveDynamicString ( "", f ); } ) );
	CHECK ( Saved ( [] ( FILE *f ) { SaveDynamicString ( OptString ( "hello" ), f ); } ) ==
	        Saved ( [] ( FILE *f ) { SaveDynamicString ( "hello", f ); } ) );
}

TEST_CASE ( "OptString loads unset, empty and text", "[optstring][save]" )
{
	FILE *f = tmpfile ();
	SaveDynamicString ( (const char *) NULL, f );
	SaveDynamicString ( "", f );
	SaveDynamicString ( "hello", f );
	rewind ( f );

	OptString a ( "junk" ), b, c;
	REQUIRE ( LoadDynamicStringPtr ( &a, f ) );
	REQUIRE ( LoadDynamicStringPtr ( &b, f ) );
	REQUIRE ( LoadDynamicStringPtr ( &c, f ) );
	fclose ( f );

	CHECK_FALSE ( a );
	CHECK ( b );
	CHECK ( b.str () == "" );
	CHECK ( c.str () == "hello" );
}
