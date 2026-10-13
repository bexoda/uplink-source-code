
/*

  News object

	Represents a single news story, posted onto 
	one of the news servers

  */


#ifndef _included_news_h
#define _included_news_h

#include "app/optstring.h"

#include <string>

// ============================================================================

#include "app/uplinkobject.h"
#include "world/date.h"

#define SIZE_NEWS_HEADLINE	128
#define SIZE_NEWS_DATA		128

#define NEWS_TYPE_NONE		0
#define NEWS_TYPE_HACKED	1				// Data1 = IP
#define NEWS_TYPE_ARREST	2				// Data1 = name of person
#define NEWS_TYPE_DELETED   3				// Data1 = IP
#define NEWS_TYPE_STOLEN	4				// Data1 = IP
#define NEWS_TYPE_COMPUTERSHUTDOWN 5		// Data1 = IP			
#define NEWS_TYPE_COMPUTERDESTROYED 6		// Data1 = IP			All databanks formatted first


// ============================================================================


class News : public UplinkObject
{

protected:

	OptString details;

public:

	Date date;
	std::string headline;
	
	int NEWSTYPE;
	std::string data1;
	std::string data2;

public:

	News ();
	~News ();

	void SetDate     ( Date *newdate );
	void SetHeadline ( const char *newheadline );
	void SetDetails  ( const char *newdetails );

	const char *GetDetails ();

	void SetData ( int newNEWSTYPE, const char *newdata1 = NULL, const char *newdata2 = NULL );

	// Common functions

	bool Load  ( FILE *file );
	void Save  ( FILE *file );
	void Print ();
	
	const char *GetID ();
	int   GetOBJECTID ();

};



#endif

