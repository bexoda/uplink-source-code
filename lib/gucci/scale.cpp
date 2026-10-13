#include "scale.h"

#include <math.h>

// Port of halveImage/scale_internal from SGI's libGLU mipmap.c (as shipped in Mesa GLU).
// gluScaleImage widens each byte to 16 bits (v * 257), filters in 16 bits, and narrows back
// with >> 8; the float/double mix below is kept exactly as in the C original so results match.

static void halve ( int components, int width, int height,
                    const unsigned char *datain, unsigned char *dataout )
{
	int newwidth = width / 2;
	int newheight = height / 2;
	int delta = width * components;
	const unsigned char *s = datain;
	unsigned char *t = dataout;

	for ( int i = 0; i < newheight; i++ ) {
		for ( int j = 0; j < newwidth; j++ ) {
			for ( int k = 0; k < components; k++ ) {
				unsigned sum = ( s[0] + s[components] + s[delta] + s[delta + components] ) * 257u;
				t[0] = (unsigned char) ( ( ( sum + 2 ) / 4 ) >> 8 );
				s++; t++;
			}
			s += components;
		}
		s += delta;
	}
}

void GciScaleRGBA ( int widthin, int heightin, const unsigned char *datain,
                    int widthout, int heightout, unsigned char *dataout )
{
	const int components = 4;

	if ( widthin == widthout * 2 && heightin == heightout * 2 ) {
		halve ( components, widthin, heightin, datain, dataout );
		return;
	}

	float convy = (float) heightin / heightout;
	float convx = (float) widthin / widthout;
	float halfconvx = convx / 2;
	float halfconvy = convy / 2;

	for ( int i = 0; i < heightout; i++ ) {
		float y = convy * ( i + 0.5 );
		float highy, lowy;
		if ( heightin > heightout ) {
			highy = y + halfconvy;
			lowy = y - halfconvy;
		} else {
			highy = y + 0.5;
			lowy = y - 0.5;
		}
		for ( int j = 0; j < widthout; j++ ) {
			float x = convx * ( j + 0.5 );
			float highx, lowx;
			if ( widthin > widthout ) {
				highx = x + halfconvx;
				lowx = x - halfconvx;
			} else {
				highx = x + 0.5;
				lowx = x - 0.5;
			}

			// Box filter over (lowx, lowy)-(highx, highy) of the input.
			float totals [4] = { 0, 0, 0, 0 };
			float area = 0;

			y = lowy;
			int yint = (int) floor ( y );
			while ( y < highy ) {
				int yindex = ( yint + heightin ) % heightin;
				float ypercent = ( highy < yint + 1 ) ? highy - y : yint + 1 - y;

				x = lowx;
				int xint = (int) floor ( x );
				while ( x < highx ) {
					int xindex = ( xint + widthin ) % widthin;
					float xpercent = ( highx < xint + 1 ) ? highx - x : xint + 1 - x;

					float percent = xpercent * ypercent;
					area += percent;
					int temp = ( xindex + ( yindex * widthin ) ) * components;
					for ( int k = 0; k < components; k++ )
						totals [k] += ( datain [temp + k] * 257 ) * percent;

					xint++;
					x = xint;
				}
				yint++;
				y = yint;
			}

			int temp = ( j + ( i * widthout ) ) * components;
			for ( int k = 0; k < components; k++ )
				dataout [temp + k] = (unsigned char) ( ( (unsigned short) ( ( totals [k] + 0.5 ) / area ) ) >> 8 );
		}
	}
}
