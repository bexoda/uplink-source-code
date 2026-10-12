# Modern build for the Uplink developer-CD source.
#
# Replaces the 2006 autopackage/apg++ makefiles. Uses the system's SDL 1.2 (sdl12-compat),
# SDL_mixer, FreeType, libtiff, libjpeg, zlib and OpenGL instead of the vendored 2006
# copies in contrib/. A few contrib pieces are still compiled from the tree: tcp4u
# (networking), irclib (in-game IRC), unrar's sha1, and FTGL 2.1.2, which Introversion
# patched and whose API the text code uses (FTGL 2.4 no longer installs FTFace.h).
#
#   cmake -S . -B build -DCMAKE_BUILD_TYPE=RelWithDebInfo
#   cmake --build build -j
#   ./build/uplink          (the .dat archives from uplink/bin are linked into the build dir)

cmake_minimum_required(VERSION 3.16)
project(Uplink LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

option(UPLINK_DEMO "Build the demo instead of the full game" OFF)
option(UPLINK_SANITIZE "Build with AddressSanitizer and UndefinedBehaviorSanitizer" OFF)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE RelWithDebInfo CACHE STRING "" FORCE)
endif()

# ---------------------------------------------------------------- dependencies

find_package(OpenGL REQUIRED)
find_package(ZLIB REQUIRED)
find_package(TIFF REQUIRED)
find_package(JPEG REQUIRED)
find_package(Freetype REQUIRED)
find_package(Threads REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(SDL REQUIRED IMPORTED_TARGET sdl)
pkg_check_modules(SDL_MIXER REQUIRED IMPORTED_TARGET SDL_mixer)

# ---------------------------------------------------------------- common flags

add_library(uplink_flags INTERFACE)
target_compile_definitions(uplink_flags INTERFACE
  _REENTRANT USE_SDL USE_FTGL
  $<IF:$<BOOL:${UPLINK_DEMO}>,DEMOGAME=1,FULLGAME=1>)
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
  # The 2003 code relies on pre-standard conversions (string literals to char*, etc.).
  # -fpermissive keeps those as warnings while the code is modernised; the long-term goal
  # is to drop it.
  target_compile_options(uplink_flags INTERFACE
    $<$<COMPILE_LANGUAGE:CXX>:-fpermissive -Wno-write-strings>)
endif()
if(UPLINK_SANITIZE)
  target_compile_options(uplink_flags INTERFACE -fsanitize=address,undefined -fno-omit-frame-pointer)
  target_link_options(uplink_flags INTERFACE -fsanitize=address,undefined)
endif()

set(LIB ${CMAKE_SOURCE_DIR}/lib)
set(CONTRIB ${CMAKE_SOURCE_DIR}/contrib)
set(SRC ${CMAKE_SOURCE_DIR}/uplink/src)

# ---------------------------------------------------------------- contrib

file(GLOB TCP4U_SOURCES ${CONTRIB}/tcp4u.331/src/*.c)
add_library(tcp4u STATIC ${TCP4U_SOURCES})
target_include_directories(tcp4u PUBLIC ${CONTRIB}/tcp4u.331/Include)
target_compile_definitions(tcp4u PRIVATE UNIX ${CMAKE_SYSTEM_NAME})
target_compile_options(tcp4u PRIVATE -w)

add_library(irclib STATIC
  ${CONTRIB}/irclib/irc.cpp ${CONTRIB}/irclib/socket.cpp
  ${CONTRIB}/irclib/CrossThreadsMessagingDevice.cpp
  ${CONTRIB}/irclib/linux/windows.cpp ${CONTRIB}/irclib/linux/missing.cpp
  ${CONTRIB}/irclib/linux/winsock.cpp)
target_include_directories(irclib PUBLIC ${CONTRIB}/irclib ${CONTRIB}/irclib/linux PRIVATE ${LIB}/mmgr)
target_link_libraries(irclib PRIVATE uplink_flags Threads::Threads)

# FTGL 2.1.2 as shipped (with Introversion's FTSize patch). The game includes it as <ftgl/...>.
file(GLOB FTGL_SOURCES ${CONTRIB}/FTGL-2.1.2/src/*.cpp)
add_library(ftgl_uplink STATIC ${FTGL_SOURCES})
file(GLOB FTGL_HEADERS ${CONTRIB}/FTGL-2.1.2/include/*.h)
file(COPY ${FTGL_HEADERS} DESTINATION ${CMAKE_BINARY_DIR}/ftgl-include/ftgl)
target_include_directories(ftgl_uplink
  PUBLIC ${CMAKE_BINARY_DIR}/ftgl-include ${CONTRIB}/FTGL-2.1.2/include)
target_link_libraries(ftgl_uplink PUBLIC Freetype::Freetype OpenGL::GL OpenGL::GLU)
target_compile_options(ftgl_uplink PRIVATE -w)

add_library(unrar_sha1 STATIC ${CONTRIB}/unrar/sha1.cpp)
target_include_directories(unrar_sha1 PUBLIC ${CONTRIB} ${CONTRIB}/unrar)
target_link_libraries(unrar_sha1 PRIVATE uplink_flags)

# ---------------------------------------------------------------- engine libraries (lib/)

# tosser is header-only: tosser.h includes its .cpp templates.
add_library(tosser INTERFACE)
target_include_directories(tosser INTERFACE ${LIB}/tosser)

add_library(mmgr STATIC ${LIB}/mmgr/mmgr.cpp)
target_include_directories(mmgr PUBLIC ${LIB}/mmgr)
target_link_libraries(mmgr PRIVATE uplink_flags)

add_library(bungle STATIC ${LIB}/bungle/bungle.cpp)
target_include_directories(bungle PUBLIC ${LIB}/bungle)
target_link_libraries(bungle PUBLIC tosser mmgr PRIVATE uplink_flags ZLIB::ZLIB)

add_library(redshirt STATIC ${LIB}/redshirt/redshirt.cpp ${LIB}/redshirt/hash.cpp)
target_include_directories(redshirt PUBLIC ${LIB}/redshirt)
target_link_libraries(redshirt PUBLIC tosser mmgr bungle unrar_sha1 PRIVATE uplink_flags)

add_library(eclipse STATIC
  ${LIB}/eclipse/animation.cpp ${LIB}/eclipse/button.cpp ${LIB}/eclipse/eclipse.cpp)
target_include_directories(eclipse PUBLIC ${LIB}/eclipse)
target_link_libraries(eclipse PUBLIC tosser mmgr gucci PRIVATE uplink_flags)

add_library(vanbakel STATIC
  ${LIB}/vanbakel/task.cpp ${LIB}/vanbakel/taskwrapper.cpp
  ${LIB}/vanbakel/interface.cpp ${LIB}/vanbakel/vanbakel.cpp)
target_include_directories(vanbakel PUBLIC ${LIB}/vanbakel)
target_link_libraries(vanbakel PUBLIC tosser mmgr eclipse PRIVATE uplink_flags)

add_library(gucci STATIC ${LIB}/gucci/gucci.cpp ${LIB}/gucci/image.cpp ${LIB}/gucci/gucci_sdl.cpp)
target_include_directories(gucci PUBLIC ${LIB}/gucci)
target_link_libraries(gucci
  PUBLIC tosser mmgr redshirt PkgConfig::SDL ftgl_uplink Freetype::Freetype OpenGL::GL OpenGL::GLU
         TIFF::TIFF JPEG::JPEG
  PRIVATE uplink_flags)

add_library(soundgarden STATIC
  ${LIB}/soundgarden/soundgarden.cpp ${LIB}/soundgarden/soundgarden_sdlmixer.cpp
  ${LIB}/soundgarden/sgplaylist.cpp)
target_include_directories(soundgarden PUBLIC ${LIB}/soundgarden)
target_link_libraries(soundgarden PUBLIC tosser mmgr redshirt eclipse PkgConfig::SDL PkgConfig::SDL_MIXER
  PRIVATE uplink_flags)

# ---------------------------------------------------------------- the game

file(GLOB_RECURSE GAME_SOURCES CONFIGURE_DEPENDS ${SRC}/*.cpp)
list(FILTER GAME_SOURCES EXCLUDE REGEX
  "/(ExceptionHandler|GetWinVer|MiniVersion|stdafx|version|resizex|template_interface)\\.cpp$")

add_executable(uplink ${GAME_SOURCES})
target_include_directories(uplink PRIVATE ${SRC} ${LIB})
target_link_libraries(uplink PRIVATE
  uplink_flags gucci soundgarden eclipse vanbakel redshirt bungle mmgr tosser
  tcp4u irclib unrar_sha1
  PkgConfig::SDL PkgConfig::SDL_MIXER ftgl_uplink Freetype::Freetype
  OpenGL::GL OpenGL::GLU TIFF::TIFF JPEG::JPEG ZLIB::ZLIB Threads::Threads ${CMAKE_DL_LIBS})

# The game reads its .dat archives from the directory holding the executable. Link them in so
# ./build/uplink runs straight from the build tree.
file(GLOB UPLINK_DATA_FILES ${CMAKE_SOURCE_DIR}/uplink/bin/*.dat)
foreach(dat ${UPLINK_DATA_FILES})
  get_filename_component(name ${dat} NAME)
  file(CREATE_LINK ${dat} ${CMAKE_BINARY_DIR}/${name} SYMBOLIC)
endforeach()
