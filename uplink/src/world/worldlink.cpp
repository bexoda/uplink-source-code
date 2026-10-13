#include <chrono>

#include "world/worldlink.h"

double WorldLink::TimeMs ()
{
	using namespace std::chrono;
	static const steady_clock::time_point start = steady_clock::now ();
	return duration<double, std::milli> ( steady_clock::now () - start ).count ();
}

static WorldLink headless;
static WorldLink *current = &headless;

WorldLink &GetWorldLink ()
{
	return *current;
}

void SetWorldLink ( WorldLink *link )
{
	current = link ? link : &headless;
}
