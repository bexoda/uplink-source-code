#include "screens.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <png.h>
#include <GL/gl.h>

#include "harness.h"

#include "uplink.h"
#include "app/app.h"
#include "app/globals.h"
#include "app/opengl.h"
#include "options/options.h"
#include "game/game.h"
#include "world/world.h"
#include "eclipse.h"

#ifndef UPLINK_TEST_SCREENS
#error UPLINK_TEST_SCREENS must point at uplink/tests/screens
#endif

namespace screens {

bool HaveDisplay ()
{
	const char *d = getenv ( "DISPLAY" );
	return d && *d;
}

void BootGraphics ()
{
	static bool booted = false;
	if ( booted ) return;
	booted = true;

	harness::Boot ();

	Options *o = app->GetOptions ();
	o->SetOptionValue ( "graphics_screenwidth", 1024 );
	o->SetOptionValue ( "graphics_screenheight", 768 );
	o->SetOptionValue ( "graphics_fullscreen", 0 );
	o->SetOptionValue ( "graphics_buttonanimations", 0 );

	// The UI animates from EclGetAccurateTime (e.g. the dashes on map connections); frames
	// step it by a fixed amount so every run draws the same pixels.
	EclSetFixedTime ( 1000000.0 );

	char arg0 [] = "uplink_screen_tests";
	char *argv [] = { arg0, nullptr };
	Init_OpenGL ( 1, argv );
}

void Frame ( int count )
{
	// Start every batch of frames on the same 1.6 s boundary, so time-driven drawing (the
	// map's dashes step every 100 or 200 ms) is in the same phase whatever ran before.
	static double now = 1000000.0;
	now = ( (long) ( now / 1600.0 ) + 2 ) * 1600.0;
	for ( int i = 0; i < count; ++i ) {
		EclSetFixedTime ( now );
		now += 40.0;
		if ( game && game->GetWorld () ) game->GetWorld ()->date.DeActivate ();
		EclUpdateAllAnimations ();
		app->Update ();
		display ();
	}
}

Image Capture ()
{
	Image img;
	img.width = app->GetOptions ()->GetOptionValue ( "graphics_screenwidth" );
	img.height = app->GetOptions ()->GetOptionValue ( "graphics_screenheight" );
	std::vector<unsigned char> raw ( img.width * img.height * 3 );
	glFinish ();
	glReadBuffer ( GL_FRONT );
	glPixelStorei ( GL_PACK_ALIGNMENT, 1 );
	glReadPixels ( 0, 0, img.width, img.height, GL_RGB, GL_UNSIGNED_BYTE, raw.data () );
	img.rgb.resize ( raw.size () );
	size_t row = img.width * 3;
	for ( int y = 0; y < img.height; ++y )
		memcpy ( &img.rgb [ y * row ], &raw [ ( img.height - 1 - y ) * row ], row );
	return img;
}

bool WritePng ( const std::string &path, const Image &img )
{
	png_image p;
	memset ( &p, 0, sizeof p );
	p.version = PNG_IMAGE_VERSION;
	p.width = img.width;
	p.height = img.height;
	p.format = PNG_FORMAT_RGB;
	return png_image_write_to_file ( &p, path.c_str (), 0, img.rgb.data (), 0, nullptr ) != 0;
}

bool ReadPng ( const std::string &path, Image &out )
{
	png_image p;
	memset ( &p, 0, sizeof p );
	p.version = PNG_IMAGE_VERSION;
	if ( !png_image_begin_read_from_file ( &p, path.c_str () ) ) return false;
	p.format = PNG_FORMAT_RGB;
	out.width = p.width;
	out.height = p.height;
	out.rgb.resize ( PNG_IMAGE_SIZE ( p ) );
	if ( !png_image_finish_read ( &p, nullptr, out.rgb.data (), 0, nullptr ) ) return false;
	return true;
}

std::string CompareWithReference ( const std::string &name, const Image &actual )
{
	std::string ref = std::string ( UPLINK_TEST_SCREENS ) + "/" + name + ".png";

	const char *update = getenv ( "UPLINK_UPDATE_SCREENS" );
	if ( update && strcmp ( update, "1" ) == 0 ) {
		if ( !WritePng ( ref, actual ) ) return "cannot write " + ref;
		return "";
	}

	Image expected;
	if ( !ReadPng ( ref, expected ) )
		return "no reference " + ref + " (run with UPLINK_UPDATE_SCREENS=1 to create it)";
	if ( expected.width != actual.width || expected.height != actual.height )
		return "size differs from " + ref;

	// A diff image: matching pixels dimmed, differing ones in red.
	Image diff = actual;
	long differing = 0;
	for ( size_t i = 0; i < actual.rgb.size (); i += 3 ) {
		bool same = actual.rgb [i] == expected.rgb [i] &&
		            actual.rgb [i+1] == expected.rgb [i+1] &&
		            actual.rgb [i+2] == expected.rgb [i+2];
		if ( same ) {
			for ( int k = 0; k < 3; ++k ) diff.rgb [i+k] = actual.rgb [i+k] / 4;
		} else {
			++differing;
			diff.rgb [i] = 255; diff.rgb [i+1] = 0; diff.rgb [i+2] = 0;
		}
	}
	if ( differing == 0 ) return "";

	WritePng ( name + ".actual.png", actual );
	WritePng ( name + ".diff.png", diff );
	char msg [256];
	snprintf ( msg, sizeof msg, "%ld pixels differ from %s; see %s.actual.png and %s.diff.png in the test directory",
	           differing, ref.c_str (), name.c_str (), name.c_str () );
	return msg;
}

}
