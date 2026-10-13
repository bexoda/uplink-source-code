
/*
    Scroll box object
    and functions

  */


#ifndef _included_scrollbox_h
#define _included_scrollbox_h

#include <string>

#include "tosser.h"


#define SIZE_SCROLLBOX_NAME     64


class ScrollBox
{

public:

    std::string name;
    int x;
    int y;
    int w;
    int h;

    int numItems;
    int windowSize;
    int currentIndex;

protected:

    void (*callback) ( const char *, int );         // Function callback for when value is changed

public:

    ScrollBox();
    ~ScrollBox();

    void SetName            ( const char *newname );
    void SetDimensions      ( int newx, int newy, int neww, int newh );
    void SetNumItems        ( int newNumItems );
    void SetWindowSize      ( int newWindowSize );
    void SetCurrentIndex    ( int newCurrentIndex );
    void SetCallback        ( void (*newcallback) (const char *, int) );

protected:

    // Interface related stuff

	bool interfaceCreated;

    void CreateInterface ();
    void RemoveInterface ();
    void UpdateInterface ();
    bool IsInterfaceVisible ();

    void ScrollUp ();
    void ScrollDown ();

    static void ClickScrollUp       ( Button *button );
    static void ClickScrollDown     ( Button *button );
    static void ClickScrollBar      ( Button *button );
    static void MouseDownScrollBar  ( Button *button );
    static void MouseMoveScrollBar  ( Button *button );
    static void DrawScrollBar       ( Button *button, bool highlighted, bool clicked );

public:

    // These manage the set of all scroll boxes in existence

    static void CreateScrollBox ( const char *name, 
                                  int x, int y, int w, int h,
                                  int numItems, int windowSize, int currentIndex,
                                  void (*callback)(const char *, int) = NULL );

    static void RemoveScrollBox ( const char *name );
    static ScrollBox *GetScrollBox ( const char *name );
    static LList <ScrollBox *> scrollBoxes;

    // Grabbing stuff

    static char currentGrab [256];
    static void GrabScrollBar   ( char *name );
    static void UnGrabScrollBar ();
    static bool IsGrabInProgress ();
    static void UpdateGrabScroll ();

};



#endif

