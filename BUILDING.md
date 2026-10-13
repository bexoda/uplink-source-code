# Building Uplink (2026)

The CMake build replaces the 2006 makefiles (`BUILDING.linux`, `BUILDING.windows.txt`), which
need the defunct autopackage `apg++` toolchain. It builds the original game unchanged in
behaviour, as C++20, against current system libraries.

## Linux

Debian/Ubuntu packages:

    sudo apt install build-essential cmake pkg-config \
        libsdl1.2-compat-dev libsdl-mixer1.2-dev libfreetype-dev \
        libtiff-dev libjpeg-dev zlib1g-dev libgl-dev libglu1-mesa-dev catch2

Build and run:

    cmake -S . -B build
    cmake --build build -j
    ./build/uplink

The game reads its `.dat` archives from the directory holding the executable; configuring
links the ones in `uplink/bin/` into the build directory. Agents, options and `debug.log`
live in `~/.uplink/`. The game sends its console output to `~/.uplink/debug.log`.

Options:

| option            | default | meaning                                              |
|-------------------|---------|------------------------------------------------------|
| `UPLINK_DEMO`     | OFF     | build the demo instead of the full game              |
| `UPLINK_TESTS`    | ON      | build `uplink_tests` (needs Catch2 3)                |
| `UPLINK_SANITIZE` | OFF     | AddressSanitizer + UndefinedBehaviorSanitizer        |

macOS (Homebrew `sdl12-compat sdl_mixer freetype libtiff jpeg catch2`) and Windows (vcpkg)
are not yet verified with this build.

## Tests

    cd build && ctest --output-on-failure

`uplink_tests` boots the real game without a window or sound (data, options, fonts) and checks:

- a world is generated, and the same seed always generates the same world;
- save → load → save is byte-identical, for a fresh world and after two game days;
- a golden SAV62 save (`uplink/tests/fixtures/`) loads and writes back unchanged, and
  generating seed 1 and simulating one day still reproduces it exactly (glibc only).

Run the suite in a slow build (Debug or sanitizers) as well as a release one. The game's
clock follows wall time, and a slow build is what showed world generation depending on it.

Tests run in a scratch `HOME` (`build/test-home`), never your `~/.uplink`. The game's own
console output from a test run is in `build/test-home/.uplink/debug.log`.

If a change alters the save format on purpose, bump `SAVEFILE_VERSION`, keep loading the old
version, and regenerate the golden file with `./uplink_tests "[.regenerate]"`.

## Sanitizers

    cmake -S . -B build-asan -DUPLINK_SANITIZE=ON -DCMAKE_BUILD_TYPE=Debug
    cmake --build build-asan -j
    cd build-asan && ASAN_OPTIONS=detect_leaks=0 ctest --output-on-failure

The game redirects stderr into `debug.log`, so sanitizer reports land there; add
`ASAN_OPTIONS=log_path=/tmp/asan` to get them in separate files. Leak checking is off
because the 2003 code frees little at exit; leaks are dealt with during the cleanup.
