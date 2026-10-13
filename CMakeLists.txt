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
  # A wrong printf argument (e.g. a std::string for %s) is undefined behaviour: an error.
  target_compile_options(uplink_flags INTERFACE -Wall -Wextra -Werror=format)
endif()
# A derived function whose signature drifts from the base virtual silently stops overriding
# it (a const change once disabled every hacking tool's SetTarget). Level 1 flags only those.
if(CMAKE_CXX_COMPILER_ID STREQUAL "GNU")
  target_compile_options(uplink_flags INTERFACE $<$<COMPILE_LANGUAGE:CXX>:-Woverloaded-virtual=1>)
elseif(CMAKE_CXX_COMPILER_ID MATCHES "Clang")
  target_compile_options(uplink_flags INTERFACE $<$<COMPILE_LANGUAGE:CXX>:-Woverloaded-virtual>)
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
target_include_directories(irclib SYSTEM PUBLIC ${CONTRIB}/irclib ${CONTRIB}/irclib/linux)
target_link_libraries(irclib PRIVATE uplink_flags Threads::Threads)
target_compile_options(irclib PRIVATE -w)

# FTGL 2.1.2 as shipped (with Introversion's FTSize patch). The game includes it as <ftgl/...>.
file(GLOB FTGL_SOURCES ${CONTRIB}/FTGL-2.1.2/src/*.cpp)
add_library(ftgl_uplink STATIC ${FTGL_SOURCES})
file(GLOB FTGL_HEADERS ${CONTRIB}/FTGL-2.1.2/include/*.h)
file(COPY ${FTGL_HEADERS} DESTINATION ${CMAKE_BINARY_DIR}/ftgl-include/ftgl)
target_include_directories(ftgl_uplink SYSTEM
  PUBLIC ${CMAKE_BINARY_DIR}/ftgl-include ${CONTRIB}/FTGL-2.1.2/include)
target_link_libraries(ftgl_uplink PUBLIC Freetype::Freetype OpenGL::GL OpenGL::GLU)
target_compile_options(ftgl_uplink PRIVATE -w)

add_library(unrar_sha1 STATIC ${CONTRIB}/unrar/sha1.cpp)
target_include_directories(unrar_sha1 SYSTEM PUBLIC ${CONTRIB} ${CONTRIB}/unrar)
target_link_libraries(unrar_sha1 PRIVATE uplink_flags)
target_compile_options(unrar_sha1 PRIVATE -w)

# ---------------------------------------------------------------- engine libraries (lib/)

# tosser is header-only: tosser.h includes its .cpp templates.
add_library(tosser INTERFACE)
target_include_directories(tosser INTERFACE ${LIB}/tosser)

add_library(bungle STATIC ${LIB}/bungle/bungle.cpp)
target_include_directories(bungle PUBLIC ${LIB}/bungle)
target_link_libraries(bungle PUBLIC tosser PRIVATE uplink_flags ZLIB::ZLIB)

add_library(redshirt STATIC ${LIB}/redshirt/redshirt.cpp ${LIB}/redshirt/hash.cpp)
target_include_directories(redshirt PUBLIC ${LIB}/redshirt)
target_link_libraries(redshirt PUBLIC tosser bungle unrar_sha1 PRIVATE uplink_flags)

add_library(eclipse STATIC
  ${LIB}/eclipse/animation.cpp ${LIB}/eclipse/button.cpp ${LIB}/eclipse/eclipse.cpp)
target_include_directories(eclipse PUBLIC ${LIB}/eclipse)
target_link_libraries(eclipse PUBLIC tosser gucci PRIVATE uplink_flags)

add_library(vanbakel STATIC
  ${LIB}/vanbakel/task.cpp ${LIB}/vanbakel/taskwrapper.cpp
  ${LIB}/vanbakel/interface.cpp ${LIB}/vanbakel/vanbakel.cpp)
target_include_directories(vanbakel PUBLIC ${LIB}/vanbakel)
target_link_libraries(vanbakel PUBLIC tosser eclipse PRIVATE uplink_flags)

add_library(gucci STATIC
  ${LIB}/gucci/gucci.cpp ${LIB}/gucci/image.cpp ${LIB}/gucci/scale.cpp ${LIB}/gucci/gucci_sdl.cpp)
target_include_directories(gucci PUBLIC ${LIB}/gucci)
target_link_libraries(gucci
  PUBLIC tosser redshirt PkgConfig::SDL ftgl_uplink Freetype::Freetype OpenGL::GL OpenGL::GLU
         TIFF::TIFF JPEG::JPEG
  PRIVATE uplink_flags)

add_library(soundgarden STATIC
  ${LIB}/soundgarden/soundgarden.cpp ${LIB}/soundgarden/soundgarden_sdlmixer.cpp
  ${LIB}/soundgarden/sgplaylist.cpp)
target_include_directories(soundgarden PUBLIC ${LIB}/soundgarden)
target_link_libraries(soundgarden PUBLIC tosser redshirt eclipse PkgConfig::SDL PkgConfig::SDL_MIXER
  PRIVATE uplink_flags)

# ---------------------------------------------------------------- the game

file(GLOB_RECURSE GAME_SOURCES CONFIGURE_DEPENDS ${SRC}/*.cpp)
list(FILTER GAME_SOURCES EXCLUDE REGEX
  "/(ExceptionHandler|GetWinVer|MiniVersion|stdafx|version|resizex|template_interface|main)\\.cpp$")

# Everything but main(), so the tests link the same code the game runs.
add_library(uplink_game OBJECT ${GAME_SOURCES})
target_include_directories(uplink_game PUBLIC ${SRC} ${LIB})
target_link_libraries(uplink_game PUBLIC
  uplink_flags gucci soundgarden eclipse vanbakel redshirt bungle tosser
  tcp4u irclib unrar_sha1
  PkgConfig::SDL PkgConfig::SDL_MIXER ftgl_uplink Freetype::Freetype
  OpenGL::GL OpenGL::GLU TIFF::TIFF JPEG::JPEG ZLIB::ZLIB Threads::Threads ${CMAKE_DL_LIBS})

add_executable(uplink ${SRC}/main.cpp)
target_link_libraries(uplink PRIVATE uplink_game)

# The game reads its .dat archives from the directory holding the executable. Link them in so
# ./build/uplink runs straight from the build tree.
file(GLOB UPLINK_DATA_FILES ${CMAKE_SOURCE_DIR}/uplink/bin/*.dat)
foreach(dat ${UPLINK_DATA_FILES})
  get_filename_component(name ${dat} NAME)
  file(CREATE_LINK ${dat} ${CMAKE_BINARY_DIR}/${name} SYMBOLIC)
endforeach()

# ---------------------------------------------------------------- tests

option(UPLINK_TESTS "Build the test suite (needs Catch2 3)" ON)
if(UPLINK_TESTS)
  find_package(Catch2 3 REQUIRED)
  enable_testing()
  set(T ${CMAKE_SOURCE_DIR}/uplink/tests)
  add_executable(uplink_tests
    ${T}/main.cpp ${T}/harness.cpp ${T}/test_world.cpp ${T}/test_golden.cpp
    ${T}/test_tools.cpp ${T}/test_options.cpp)
  target_link_libraries(uplink_tests PRIVATE uplink_game Catch2::Catch2 ZLIB::ZLIB)
  target_compile_definitions(uplink_tests PRIVATE
    UPLINK_TEST_FIXTURES="${CMAKE_SOURCE_DIR}/uplink/tests/fixtures")
  file(MAKE_DIRECTORY ${CMAKE_BINARY_DIR}/test-home)
  include(Catch)
  # Tests boot the real game from the build dir (where the .dat files are linked) and keep
  # their agent files in a scratch HOME, never the developer's ~/.uplink.
  catch_discover_tests(uplink_tests
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    PROPERTIES ENVIRONMENT "HOME=${CMAKE_BINARY_DIR}/test-home")

  # Screenshot tests need a window: they skip without DISPLAY. CI runs them under xvfb-run.
  find_package(PNG REQUIRED)
  add_executable(uplink_screen_tests
    ${T}/main.cpp ${T}/harness.cpp ${T}/screens.cpp ${T}/test_screens.cpp)
  target_link_libraries(uplink_screen_tests PRIVATE uplink_game Catch2::Catch2 PNG::PNG)
  target_compile_definitions(uplink_screen_tests PRIVATE
    UPLINK_TEST_SCREENS="${CMAKE_SOURCE_DIR}/uplink/tests/screens")
  catch_discover_tests(uplink_screen_tests
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    PROPERTIES ENVIRONMENT "HOME=${CMAKE_BINARY_DIR}/test-home" LABELS screens
               SKIP_RETURN_CODE 4)
endif()
