// RGBA8 image scaling on the CPU.
//
// Same results as gluScaleImage(GL_RGBA, ..., GL_UNSIGNED_BYTE, ...) with default pixel-store
// settings (it is a port of the SGI GLU box filter), but needs no OpenGL context. The world
// generator scales the world-map mask with it, so generation can run without a window.

#ifndef _included_gucci_scale_h
#define _included_gucci_scale_h

void GciScaleRGBA ( int widthin, int heightin, const unsigned char *datain,
                    int widthout, int heightout, unsigned char *dataout );

#endif
