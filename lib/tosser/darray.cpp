// Tosser implementation file
// Part of tosser
// By Christopher Delay
//
// Storage is two std::vectors (the data, each in a one-member struct, and a used flag per slot). Size () still grows by the
// step size, as callers loop over it; the vectors' capacity grows geometrically underneath.
// firstfree is the lowest slot that may be unused, so PutData does not rescan the used prefix.

#ifndef _included_tosser_darray
#define _included_tosser_darray

#include <algorithm>
#include <cstdlib>
#include <iostream>
#include <assert.h>

#include "tosser.h"

template <class T>
DArray <T> :: DArray ()
	: stepsize ( 1 ), firstfree ( 0 )
{
}

template <class T>
DArray <T> :: DArray ( int newstepsize )
	: stepsize ( newstepsize ), firstfree ( 0 )
{
}

template <class T>
void DArray <T> :: SetSize ( int newsize )
{

	if ( newsize <= 0 ) {
		Empty ();
		return;
	}

	dynarray.resize ( newsize );
	shadow.resize ( newsize, 0 );
	firstfree = std::min ( firstfree, newsize );

}

template <class T>
void DArray <T> :: SetStepSize ( int newstepsize )
{

	stepsize = newstepsize;

}

template <class T>
void DArray <T> :: Reserve ( int capacity )
{

	dynarray.reserve ( capacity );
	shadow.reserve ( capacity );

}

template <class T>
int DArray <T> :: PutData ( const T &newdata )
{

	int size = Size ();
	int freespace = firstfree;
	while ( freespace < size && shadow [freespace] )
		++freespace;

	if ( freespace == size )					// Must resize the array
		SetSize ( size + stepsize );

	dynarray [freespace].value = newdata;
	shadow [freespace] = 1;
	firstfree = freespace + 1;

	return freespace;

}

template <class T>
void DArray <T> :: PutData ( const T &newdata, int index )
{

	assert ( index < Size () && index >= 0 );

	dynarray [index].value = newdata;
	shadow [index] = 1;

}

template <class T>
void DArray <T> :: Empty ()
{

	dynarray.clear ();
	dynarray.shrink_to_fit ();
	shadow.clear ();
	shadow.shrink_to_fit ();
	firstfree = 0;

}

template <class T>
T DArray <T> :: GetData ( int index ) const
{

	assert ( index < Size () && index >= 0 );

	if ( shadow [index] == 0 )
		std::cout << "DArray::GetData called, referenced unused data.  (Index = " << index << ")\n";

	return dynarray [index].value;

}

template <class T>
T& DArray <T> :: operator [] (int index)
{

	assert ( index < Size () && index >= 0 );

	if ( shadow [index] == 0 )
		std::cout << "DArray error : DArray::[] called, referenced unused data.  (Index = " << index << ")\n";

	return dynarray [index].value;
}

template <class T>
void DArray <T> :: ChangeData ( const T &newdata, int index )
{

	assert ( index < Size () && index >= 0 );

	if ( shadow [index] == 0 )
		std::cout << "Warning : DArray::ChangeData called, referenced unused data.  (Index = " << index << ")\n";

	PutData ( newdata, index );

}

template <class T>
void DArray <T> :: RemoveData ( int index )
{

	assert ( index < Size () && index >= 0 );

	if ( shadow [index] == 0 )
		std::cout << "Warning : DArray::RemoveData called, referenced unused data.  (Index = " << index << ")\n";

	shadow [index] = 0;
	firstfree = std::min ( firstfree, index );

}

template <class T>
int DArray <T> :: NumUsed () const
{

	return (int) std::count ( shadow.begin (), shadow.end (), 1 );

}

template <class T>
int DArray <T> :: Size () const
{

	return (int) dynarray.size ();

}

template <class T>
bool DArray <T> :: ValidIndex ( int index ) const
{

	return index >= 0 && index < Size () && shadow [index];

}

template <class T>
int DArray <T> :: FindData ( const T &newdata ) const
{

	for ( int a = 0; a < Size (); ++a )
		if ( shadow [a] && dynarray [a].value == newdata )
			return a;

	return -1;

}

template <class T>
void DArray <T> :: Sort ( Comparator comp )
{

	// Move the used entries to the front, keeping their order, then sort them

	int used = 0;
	for ( int a = 0; a < Size (); ++a ) {
		if ( shadow [a] ) {
			if ( a != used ) {
				dynarray [used] = dynarray [a];
				shadow [used] = 1;
				shadow [a] = 0;
			}
			++used;
		}
	}
	firstfree = used;

	static_assert ( sizeof ( Slot ) == sizeof ( T ), "the comparator sees each Slot as a T" );
	if ( used > 0 )
		qsort ( dynarray.data (), used, sizeof ( Slot ), ( int (*)(const void *,const void *) )comp );

}

#endif
