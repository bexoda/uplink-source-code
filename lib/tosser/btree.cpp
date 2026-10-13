
// BTree implementation file
// Part of Tosser
// By Christopher Delay
// Modifications John Knottenbelt

#ifndef _included_tosser_btree
#define _included_tosser_btree

#include "tosser.h"

#include <assert.h>
#include <string.h>


template <class T>
BTree <T> :: BTree ()
{

    ltree = NULL;
    rtree = NULL;
    id = NULL;
    //data = NULL;
    data = 0;

}

template <class T>
BTree <T> :: BTree ( const char *newid, const T &newdata )
{
        
    ltree = NULL;
    rtree = NULL;
    id = new char [ strlen (newid) + 1 ];
    strcpy ( id, newid );
    data = newdata;
    
}


template <class T>
BTree<T> :: BTree ( const BTree<T> &copy )
{
    Copy( copy );
}

template <class T>
void BTree<T> :: Copy( const BTree<T> &copy )
{
    if (copy.ltree)	ltree = new BTree( *copy.ltree );
    else ltree = NULL;
    
    if (copy.rtree)	rtree = new BTree( *copy.rtree );
    else rtree = NULL;
    
    if (copy.id) {
		id = new char [ strlen (copy.id) + 1 ];
		strcpy ( id, copy.id );
    }
    else 
		id = NULL;
    
    data = copy.data;
}


template <class T>
BTree <T> :: ~BTree ()
{
    Empty ();
}

template <class T>
void BTree <T> :: Empty ()
{
    if (ltree) delete ltree;
    if (rtree) delete rtree;
    if (id)    delete [] id;
    
    ltree = rtree = NULL;
    id = NULL;
}

template <class T>
void BTree <T> :: PutData ( const char *newid, const T &newdata )
{
    
    if ( !id ) {

        id = new char [ strlen ( newid ) + 1 ];
        strcpy ( id, newid );
        data = newdata;
	 	 
		return;

    }

    if ( strcmp ( newid, id ) <= 0 ) {

        if (ltree)
            ltree->PutData ( newid, newdata );

        else
            ltree = new BTree ( newid, newdata );

    }
    else if ( strcmp ( newid, id ) > 0 ) {

        if (rtree)
            rtree->PutData ( newid, newdata );

        else
            rtree = new BTree ( newid, newdata );

    }

}

// Replaces this node by its left child (the old right subtree goes on the far right of
// it), or by its right child, or empties it. Frees what it takes out.
template <class T>
void BTree <T> :: RemoveThisNode ()
{

    BTree <T> *tempright = rtree;
    delete [] id;

    if ( ltree ) {
        BTree <T> *left = ltree;
        id = left->id;
        data = left->data;
        ltree = left->ltree;
        rtree = left->rtree;
        left->id = NULL;
        left->ltree = left->rtree = NULL;
        delete left;
        AppendRight ( tempright );
    }
    else if ( rtree ) {
        BTree <T> *right = rtree;
        id = right->id;
        data = right->data;
        ltree = right->ltree;
        rtree = right->rtree;
        right->id = NULL;
        right->ltree = right->rtree = NULL;
        delete right;
    }
    else {
        id = NULL;                              // Hopefully this is the root node
    }

}

template <class T>
void BTree <T> :: RemoveData ( const char *newid )
{

    assert (newid);
    if ( !id ) return;

    int cmp = strcmp ( newid, id );

    if ( cmp == 0 ) {
        RemoveThisNode ();
    }
    else if ( cmp < 0 ) {
        if ( ltree ) {
            if ( ltree->id && strcmp ( ltree->id, newid ) == 0 && !ltree->ltree && !ltree->rtree ) {
                delete ltree;
                ltree = NULL;
            }
            else
                ltree->RemoveData ( newid );
        }
    }
    else {
        if ( rtree ) {
            if ( rtree->id && strcmp ( rtree->id, newid ) == 0 && !rtree->ltree && !rtree->rtree ) {
                delete rtree;
                rtree = NULL;
            }
            else
                rtree->RemoveData ( newid );
        }
    }

}

template <class T>
void BTree <T> :: RemoveData ( const char *newid, const T &newdata )
{

    assert (newid);
    if ( !id ) return;

    int cmp = strcmp ( newid, id );

    if ( cmp == 0 && data == newdata ) {
        RemoveThisNode ();
    }
    else if ( cmp <= 0 ) {
        if ( ltree ) {
            if ( ltree->id && strcmp ( ltree->id, newid ) == 0 && ltree->data == newdata && !ltree->ltree && !ltree->rtree ) {
                delete ltree;
                ltree = NULL;
            }
            else
                ltree->RemoveData ( newid, newdata );
        }
    }
    else {
        if ( rtree ) {
            if ( rtree->id && strcmp ( rtree->id, newid ) == 0 && rtree->data == newdata && !rtree->ltree && !rtree->rtree ) {
                delete rtree;
                rtree = NULL;
            }
            else
                rtree->RemoveData ( newid, newdata );
        }
    }

}

template <class T>
T BTree <T> :: GetData ( const char *searchid )
{

	BTree <T> *btree = LookupTree ( searchid );

	if ( btree ) return btree->data;
	else return T ();

}


template <class T>
void BTree <T> :: AppendRight ( BTree <T> *tempright )
{
    
    if ( !Right () )
		rtree = tempright;
    
    else
		Right ()->AppendRight ( tempright );	
    
}

template <class T>
BTree<T> *BTree<T> :: LookupTree( const char *searchid )
{
        
    if (!id)
		return NULL;
    
    if ( strcmp ( searchid, id ) == 0 )
        return this;

    else if ( ltree && strcmp ( searchid, id ) < 0 )
        return ltree->LookupTree ( searchid );

    else if ( rtree && strcmp ( searchid, id ) > 0 )
        return rtree->LookupTree ( searchid );
    
    else return NULL;
    
}


template <class T>
int BTree <T> :: Size () const
{

    unsigned int subsize = (id) ? 1 : 0;
    
    if (ltree) subsize += ltree->Size ();
    if (rtree) subsize += rtree->Size ();

    return subsize;

}

template <class T>
void BTree <T> :: Print ()
{
    
    if (ltree) ltree->Print ();
    if (id) cout << id << " : " << data << "\n";       
    if (rtree) rtree->Print ();
    
}
    
template <class T>
BTree <T> *BTree <T> :: Left () const
{

    return (BTree <T>*) ltree;

}

template <class T>
BTree <T> *BTree <T> :: Right () const
{

    return (BTree <T>*) rtree;

}

template <class T>
DArray <T> *BTree <T> :: ConvertToDArray ()
{

    std::vector <T> items;
    CollectData ( items );

    DArray <T> *darray = new DArray <T>;
    darray->Reserve ( (int) items.size () );
    for ( const T &item : items )
        darray->PutData ( item );

    return darray;

}

template <class T>
DArray <char *> *BTree <T> :: ConvertIndexToDArray ()
{

    std::vector <char *> ids;
    CollectIds ( ids );

    DArray <char *> *darray = new DArray <char *>;
    darray->Reserve ( (int) ids.size () );
    for ( char *item : ids )
        darray->PutData ( item );

    return darray;

}

// Node, then left, then right: the order saves and updates have always used.

template <class T>
void BTree <T> :: CollectData ( std::vector <T> &out ) const
{

    if ( id ) out.push_back ( data );
    if ( ltree ) ltree->CollectData ( out );
    if ( rtree ) rtree->CollectData ( out );

}

template <class T>
void BTree <T> :: CollectIds ( std::vector <char *> &out ) const
{

    if ( id ) out.push_back ( id );
    if ( ltree ) ltree->CollectIds ( out );
    if ( rtree ) rtree->CollectIds ( out );

}

#endif
