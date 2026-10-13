// Screenshot regression tests: the real game rendered into a real window (Xvfb in CI),
// captured, and compared with reference PNGs in uplink/tests/screens/.

#pragma once

#include <string>
#include <vector>

namespace screens {

struct Image {
	int width = 0, height = 0;
	std::vector<unsigned char> rgb;			// top row first
};

// False when there is no display to open a window on; tests SKIP then.
bool HaveDisplay ();

// Boot() plus a game window with animations off, so a frame is final as soon as it is drawn.
void BootGraphics ();

// One pass of the game loop (animations, update, draw) with the world clock frozen.
void Frame ( int count = 1 );

Image Capture ();

bool ReadPng ( const std::string &path, Image &out );
bool WritePng ( const std::string &path, const Image &img );

// Compares `actual` with the reference `name`.png. On mismatch writes name.actual.png and
// name.diff.png next to the build's test output and returns a description. With
// UPLINK_UPDATE_SCREENS=1 in the environment the reference is (re)written instead.
std::string CompareWithReference ( const std::string &name, const Image &actual );

}
