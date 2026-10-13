// Brings the real game up without a window or sound, for tests.
//
// The game keeps its state in globals (app, game), so there is one game per test process.
// Boot() is idempotent; NewWorld() throws away the current game and generates a new one.

#pragma once

#include <string>
#include <vector>

namespace harness {

// Creates the App, loads options and the .dat archives, creates the Game. Run from the build
// directory (CTest does), where the archives are linked.
void Boot ();

// Generates a fresh world from `seed`, exactly as "new agent" does.
void NewWorld ( unsigned seed );

// Saves the current game the way the game does (unencrypted body) and returns the bytes.
std::vector<unsigned char> SaveBytes ();

// Replaces the current game with one loaded from `bytes`. Returns false if the loader did.
bool LoadBytes ( const std::vector<unsigned char> &bytes );

// Runs the world simulation forward by `gameSeconds` of game time.
void RunWorld ( int gameSeconds );

std::string ScratchFile ( const char *name );

// Frees the game and every registered button, so LeakSanitizer reports only real leaks.
void Shutdown ();

}
