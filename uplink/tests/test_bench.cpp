// Timings for world generation and simulation. Hidden: run with  ./uplink_tests "[.bench]"
// The numbers go to the report as messages; nothing is asserted beyond the run completing.

#include <catch2/catch_test_macros.hpp>
#include <catch2/catch_message.hpp>

#include <chrono>
#include <cstdio>
#include <cstdlib>

#include "harness.h"

namespace {

double Seconds ( std::chrono::steady_clock::time_point since )
{
	return std::chrono::duration<double> ( std::chrono::steady_clock::now () - since ).count ();
}

}

TEST_CASE ( "benchmark: generate a world, then simulate a game day", "[.bench]" )
{
	harness::Boot ();

	auto t0 = std::chrono::steady_clock::now ();
	harness::NewWorld ( 1 );
	double generate = Seconds ( t0 );

	auto t1 = std::chrono::steady_clock::now ();
	const char *secs = getenv ( "UPLINK_BENCH_SECONDS" );		// shorter runs for profilers
	harness::RunWorld ( secs ? atoi ( secs ) : 24 * 60 * 60 );
	double simulate = Seconds ( t1 );

	auto t2 = std::chrono::steady_clock::now ();
	size_t bytes = harness::SaveBytes ().size ();
	double save = Seconds ( t2 );

	char line [256];
	snprintf ( line, sizeof line, "generate %.3fs  simulate %.3fs  save %.3fs (%zu bytes)",
	           generate, simulate, save, bytes );
	WARN ( line );
	SUCCEED ();
}
