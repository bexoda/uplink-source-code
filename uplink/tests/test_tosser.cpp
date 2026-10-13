// Tosser containers (LList, DArray, BTree): the behaviour the game relies on, including the
// odd edge cases, so the implementations can change underneath without the game noticing.

#include <catch2/catch_test_macros.hpp>

#include <cstring>
#include <string>
#include <vector>

#include "tosser.h"

namespace {

template <class T> std::vector<T> Items ( LList<T> &l )
{
	std::vector<T> out;
	for ( int i = 0; i < l.Size (); ++i ) out.push_back ( l.GetData ( i ) );
	return out;
}

template <class T> std::vector<T> Used ( DArray<T> &d )
{
	std::vector<T> out;
	for ( int i = 0; i < d.Size (); ++i ) if ( d.ValidIndex ( i ) ) out.push_back ( d.GetData ( i ) );
	return out;
}

std::vector<std::string> Ids ( BTree<int> &t )
{
	DArray<char *> *d = t.ConvertIndexToDArray ();
	std::vector<std::string> out;
	for ( int i = 0; i < d->Size (); ++i ) if ( d->ValidIndex ( i ) ) out.push_back ( d->GetData ( i ) );
	delete d;
	return out;
}

std::vector<int> Values ( BTree<int> &t )
{
	DArray<int> *d = t.ConvertToDArray ();
	std::vector<int> out = Used ( *d );
	delete d;
	return out;
}

}

// ---------------------------------------------------------------- LList

TEST_CASE ( "LList: ordered inserts, random and sequential reads", "[tosser]" )
{
	LList<int> l;
	CHECK ( l.Size () == 0 );
	CHECK ( l.GetData ( 0 ) == 0 );				// out of range reads give T(0)
	l.PutData ( 2 );
	l.PutDataAtEnd ( 3 );
	l.PutDataAtStart ( 1 );
	l.PutDataAtIndex ( 9, 1 );					// before index 1
	l.PutDataAtIndex ( 4, l.Size () );			// at the end
	CHECK ( Items ( l ) == std::vector<int> { 1, 9, 2, 3, 4 } );
	CHECK ( l.GetData ( 3 ) == 3 );				// random
	CHECK ( l.GetData ( 3 ) == 3 );				// same again
	CHECK ( l.GetData ( 4 ) == 4 );				// next
	CHECK ( l [1] == 9 );
	CHECK ( l.GetData ( 5 ) == 0 );
	CHECK ( l.GetData ( -1 ) == 1 );				// a negative index reads the first item
	CHECK ( l.ValidIndex ( 4 ) );
	CHECK_FALSE ( l.ValidIndex ( 5 ) );
	CHECK_FALSE ( l.ValidIndex ( -1 ) );
	CHECK ( l.FindData ( 2 ) == 2 );
	CHECK ( l.FindData ( 7 ) == -1 );
}

TEST_CASE ( "LList: removing, and inserts past the end", "[tosser]" )
{
	LList<int> l;
	for ( int i = 0; i < 5; ++i ) l.PutData ( i );
	l.RemoveData ( 2 );
	CHECK ( Items ( l ) == std::vector<int> { 0, 1, 3, 4 } );
	l.RemoveData ( 9 );							// out of range: nothing
	CHECK ( l.Size () == 4 );
	l.RemoveData ( -3 );							// negative: removes the first
	CHECK ( Items ( l ) == std::vector<int> { 1, 3, 4 } );
	l.PutDataAtIndex ( 7, 9 );					// past the end: ignored
	CHECK ( Items ( l ) == std::vector<int> { 1, 3, 4 } );
	l.PutDataAtIndex ( 8, -2 );					// negative, not 0: goes after the first
	CHECK ( Items ( l ) == std::vector<int> { 1, 8, 3, 4 } );
	l.RemoveData ( 3 );
	l.RemoveData ( 0 );
	CHECK ( Items ( l ) == std::vector<int> { 8, 3 } );
	l.PutData ( 5 );							// last still right after removing the tail
	CHECK ( Items ( l ) == std::vector<int> { 8, 3, 5 } );
	l.Empty ();
	CHECK ( l.Size () == 0 );
	l.PutDataAtIndex ( 1, 3 );					// empty and not 0: ignored
	CHECK ( l.Size () == 0 );
	l.PutDataAtIndex ( 1, 0 );
	CHECK ( Items ( l ) == std::vector<int> { 1 } );
}

TEST_CASE ( "LList: reads interleaved with writes stay correct", "[tosser]" )
{
	LList<int> l;
	for ( int i = 0; i < 10; ++i ) l.PutData ( i );
	CHECK ( l.GetData ( 4 ) == 4 );
	CHECK ( l.GetData ( 5 ) == 5 );
	l.RemoveData ( 6 );
	CHECK ( l.GetData ( 6 ) == 7 );
	l.PutDataAtStart ( -1 );
	CHECK ( l.GetData ( 7 ) == 7 );
	CHECK ( l.GetData ( 8 ) == 8 );
}

// ---------------------------------------------------------------- DArray

TEST_CASE ( "DArray: indexes are stable and holes are reused lowest first", "[tosser]" )
{
	DArray<int> d;
	CHECK ( d.Size () == 0 );
	CHECK ( d.PutData ( 10 ) == 0 );
	CHECK ( d.PutData ( 11 ) == 1 );
	CHECK ( d.PutData ( 12 ) == 2 );
	CHECK ( d.Size () == 3 );					// step size 1
	d.RemoveData ( 1 );
	d.RemoveData ( 0 );
	CHECK_FALSE ( d.ValidIndex ( 0 ) );
	CHECK ( d.ValidIndex ( 2 ) );
	CHECK ( d.GetData ( 2 ) == 12 );			// others keep their index
	CHECK ( d.NumUsed () == 1 );
	CHECK ( d.PutData ( 20 ) == 0 );			// lowest hole first
	CHECK ( d.PutData ( 21 ) == 1 );
	CHECK ( d.PutData ( 22 ) == 3 );
	CHECK ( d.Size () == 4 );
	CHECK ( d.FindData ( 21 ) == 1 );
	CHECK ( d.FindData ( 99 ) == -1 );
	d.ChangeData ( 30, 3 );
	CHECK ( d [3] == 30 );
	d [3] = 31;
	CHECK ( d.GetData ( 3 ) == 31 );
	CHECK_FALSE ( d.ValidIndex ( 4 ) );
	CHECK_FALSE ( d.ValidIndex ( -1 ) );
}

TEST_CASE ( "DArray: step size, SetSize and explicit placement", "[tosser]" )
{
	DArray<int> d ( 4 );
	d.PutData ( 1 );
	CHECK ( d.Size () == 4 );
	for ( int i = 0; i < 3; ++i ) d.PutData ( i );
	CHECK ( d.Size () == 4 );
	d.PutData ( 9 );
	CHECK ( d.Size () == 8 );
	d.SetStepSize ( 2 );
	for ( int i = 0; i < 4; ++i ) d.PutData ( i );
	CHECK ( d.Size () == 10 );

	d.SetSize ( 3 );							// shrink keeps the first entries
	CHECK ( d.Size () == 3 );
	CHECK ( Used ( d ) == std::vector<int> { 1, 0, 1 } );
	d.SetSize ( 6 );							// grow: new slots unused
	CHECK ( d.NumUsed () == 3 );
	CHECK_FALSE ( d.ValidIndex ( 4 ) );
	d.PutData ( 7, 5 );
	CHECK ( d.ValidIndex ( 5 ) );
	CHECK ( d.PutData ( 8 ) == 3 );			// holes below 5 are still found
	d.SetSize ( 0 );
	CHECK ( d.Size () == 0 );
	CHECK ( d.PutData ( 1 ) == 0 );
}

TEST_CASE ( "DArray: copies are deep, Sort compacts then sorts", "[tosser]" )
{
	DArray<int> d;
	for ( int v : { 5, 3, 9, 1 } ) d.PutData ( v );
	d.RemoveData ( 1 );

	DArray<int> c ( d );
	c.PutData ( 7 );							// fills the copy's hole only
	CHECK ( d.NumUsed () == 3 );
	CHECK ( c.NumUsed () == 4 );

	d.Sort ( [] ( const int *a, const int *b ) { return *a - *b; } );
	CHECK ( d.Size () == 4 );
	CHECK ( Used ( d ) == std::vector<int> { 1, 5, 9 } );
	CHECK_FALSE ( d.ValidIndex ( 3 ) );

	d.Empty ();
	CHECK ( d.Size () == 0 );
	CHECK ( d.NumUsed () == 0 );
}

TEST_CASE ( "DArray: many inserts after removals", "[tosser]" )
{
	DArray<int> d;
	for ( int i = 0; i < 1000; ++i ) CHECK ( d.PutData ( i ) == i );
	for ( int i = 0; i < 1000; i += 3 ) d.RemoveData ( i );
	int expect = 0;
	for ( int i = 0; i < 334; ++i, expect += 3 ) CHECK ( d.PutData ( -i ) == expect );
	CHECK ( d.PutData ( 5000 ) == 1000 );
	CHECK ( d.NumUsed () == 1001 );
}

// ---------------------------------------------------------------- BTree

TEST_CASE ( "BTree: lookups, duplicates and pre-order conversion", "[tosser]" )
{
	BTree<int> t;
	CHECK ( t.Size () == 0 );
	CHECK ( t.LookupTree ( "x" ) == NULL );
	for ( auto [id, v] : std::vector<std::pair<const char *, int>> {
	          { "m", 1 }, { "c", 2 }, { "x", 3 }, { "a", 4 }, { "e", 5 }, { "c", 6 } } )
		t.PutData ( id, v );
	CHECK ( t.Size () == 6 );
	CHECK ( t.GetData ( "e" ) == 5 );
	CHECK ( t.GetData ( "c" ) == 2 );			// the first one put
	CHECK ( t.GetData ( "zz" ) == 0 );
	// Saves walk the tree node, left, right: the order depends on the shape
	CHECK ( Ids ( t ) == std::vector<std::string> { "m", "c", "a", "c", "e", "x" } );
	CHECK ( Values ( t ) == std::vector<int> { 1, 2, 4, 6, 5, 3 } );

	BTree<int> *node = t.LookupTree ( "x" );
	REQUIRE ( node );
	node->data = 33;
	CHECK ( t.GetData ( "x" ) == 33 );

	BTree<int> copy ( t );
	copy.PutData ( "b", 9 );
	CHECK ( copy.Size () == 7 );
	CHECK ( t.Size () == 6 );
}

TEST_CASE ( "BTree: removing", "[tosser]" )
{
	BTree<int> t;
	for ( auto [id, v] : std::vector<std::pair<const char *, int>> {
	          { "m", 1 }, { "c", 2 }, { "x", 3 }, { "a", 4 }, { "e", 5 }, { "q", 6 }, { "z", 7 } } )
		t.PutData ( id, v );

	t.RemoveData ( "a" );						// a leaf
	CHECK ( t.Size () == 6 );
	CHECK ( t.LookupTree ( "a" ) == NULL );
	t.RemoveData ( "x" );						// two children
	CHECK ( t.Size () == 5 );
	CHECK ( t.GetData ( "q" ) == 6 );
	CHECK ( t.GetData ( "z" ) == 7 );
	t.RemoveData ( "m" );						// the root
	CHECK ( t.Size () == 4 );
	CHECK ( Ids ( t ) == std::vector<std::string> { "c", "e", "q", "z" } );
	t.RemoveData ( "q", 99 );					// wrong data: stays
	CHECK ( t.Size () == 4 );
	t.RemoveData ( "q", 6 );
	CHECK ( Ids ( t ) == std::vector<std::string> { "c", "e", "z" } );
	for ( const char *id : { "c", "e", "z" } ) t.RemoveData ( id );
	CHECK ( t.Size () == 0 );
	t.PutData ( "n", 1 );
	CHECK ( t.GetData ( "n" ) == 1 );
}

TEST_CASE ( "BTree: RemoveData ( id, data ) checks the data of the node it removes", "[tosser]" )
{
	BTree<int> t;
	t.PutData ( "m", 1 );
	t.PutData ( "c", 2 );						// a leaf under "m"
	t.RemoveData ( "c", 1 );					// "m" holds 1, "c" does not: nothing to remove
	CHECK ( t.GetData ( "c" ) == 2 );
	t.RemoveData ( "c", 2 );
	CHECK ( t.LookupTree ( "c" ) == NULL );
	CHECK ( t.Size () == 1 );
}
