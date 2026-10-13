// The task scheduler (vanbakel) owns the tasks registered with it.

#include <catch2/catch_test_macros.hpp>

#include "harness.h"

#include "task.h"
#include "vanbakel.h"

namespace {

int destroyed = 0;

struct CountedTask : public Task
{
	~CountedTask () override { ++destroyed; }
	void Tick ( int /*n*/ ) override {}
	void CreateInterface () override {}
	void RemoveInterface () override {}
};

}

TEST_CASE ( "SvbReset deletes every registered task", "[scheduler]" )
{
	harness::Boot ();
	SvbReset ();
	destroyed = 0;

	for ( int i = 0; i < 5; ++i )
		SvbRegisterTask ( "counted", new CountedTask () );
	REQUIRE ( SvbNumTasks () == 5 );

	SvbReset ();
	CHECK ( SvbNumTasks () == 0 );
	CHECK ( destroyed == 5 );		// an index walk over the shrinking list freed only 3
}
