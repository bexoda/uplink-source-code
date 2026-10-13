// Tosser implementation file
// Part of tosser
// By Christopher Delay
//
// Stored in a std::vector: random access is as fast as sequential access now. The edge
// cases of the original linked list are kept (see uplink/tests/test_tosser.cpp): reads and
// removals at a negative index act on the first item, reads out of range give T (), and an
// insert past the end is ignored.

#ifndef _included_tosser_llist
#define _included_tosser_llist

#include <algorithm>
#include <assert.h>

#include "tosser.h"

template <class T>
void LList<T>::PutData ( const T &newdata )
{

    PutDataAtEnd ( newdata );

}

template <class T>
void LList<T>::PutDataAtEnd ( const T &newdata )
{

    items.push_back ( newdata );

}

template <class T>
void LList<T>::PutDataAtStart ( const T &newdata )
{

    items.insert ( items.begin (), newdata );

}

template <class T>
void LList <T>::PutDataAtIndex ( const T &newdata, int index )
{

    int size = Size ();

    if ( index == 0 ) {
        PutDataAtStart ( newdata );
    }
    else if ( index == size ) {
        PutDataAtEnd ( newdata );
    }
    else {
        int after = std::max ( index - 1, 0 );			// goes after this item, if it exists
        if ( after < size )
            items.insert ( items.begin () + after + 1, newdata );
    }

}

template <class T>
int LList <T>::Size ()
{

    return (int) items.size ();

}

template <class T>
T LList <T>::GetData ( int index )
{

    int at = std::max ( index, 0 );
    if ( at >= Size () ) return T ();
    return items [at];

}

template <class T>
T LList <T> :: operator [] (int index)
{

    return GetData (index);

}

template <class T>
bool LList <T>::ValidIndex ( int index )
{

    return ( index >= 0 && index < Size () );

}

template <class T>
void LList <T>::Empty ()
{

    items.clear ();
    items.shrink_to_fit ();

}

template <class T>
void LList <T>::RemoveData ( int index )
{

    int at = std::max ( index, 0 );
    if ( at < Size () )
        items.erase ( items.begin () + at );

}

template <class T>
int LList <T>::FindData ( const T &data )
{

    for ( int i = 0; i < Size (); ++i )
        if ( items [i] == data )
            return i;

    return -1;

}

#endif
