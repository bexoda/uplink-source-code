// OptString: an owned string that can also be unset.
//
// Replaces the game's heap-allocated, nullable char * members. Unset plays the role NULL did:
// c_str () returns NULL, it tests false, and it is saved as "no string" (length -1), so the save
// format is unchanged. Assigning NULL unsets it.

#ifndef _included_optstring_h
#define _included_optstring_h

#include <cstddef>
#include <string>

class OptString
{
public:

	OptString () = default;
	OptString ( const char *s ) { *this = s; }
	OptString ( const std::string &s ) : value ( s ), set ( true ) {}

	OptString &operator= ( const char *s )
	{
		if ( s ) { value = s; set = true; }
		else     { value.clear (); set = false; }
		return *this;
	}
	OptString &operator= ( const std::string &s ) { value = s; set = true; return *this; }

	const char *c_str () const { return set ? value.c_str () : NULL; }		// NULL when unset
	explicit operator bool () const { return set; }

	bool empty () const { return value.empty (); }
	std::size_t size () const { return value.size (); }
	const std::string &str () const { return value; }							// "" when unset

private:

	std::string value;
	bool set = false;

};

#endif
