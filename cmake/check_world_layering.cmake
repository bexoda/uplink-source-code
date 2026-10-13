# The simulation (uplink/src/world) must not reach the screen, sound or UI libraries
# directly: it goes through WorldLink (world/worldlink.h). Run by CTest.
#   cmake -DSRC=<uplink/src> -P check_world_layering.cmake

file(GLOB_RECURSE files "${SRC}/world/*.cpp" "${SRC}/world/*.h")
set(bad_include [=[#include "(interface/|mainmenu/|view/|app/opengl|eclipse\.h|gucci\.h|soundgarden\.h|vanbakel\.h|svbinterface\.h)]=])
set(bad_call [=[(^|[^A-Za-z0-9_])(Ecl[A-Z][A-Za-z0-9_]*|Gci[A-Z][A-Za-z0-9_]*|Sg[A-Z][A-Za-z0-9_]*|Svb[A-Z][A-Za-z0-9_]*|GetInterface|GetMainMenu|create_msgbox) *\(]=])

set(problems "")
foreach(f ${files})
  file(STRINGS "${f}" lines)
  set(n 0)
  foreach(line IN LISTS lines)
    math(EXPR n "${n} + 1")
    string(REGEX REPLACE "//.*$" "" code "${line}")
    if(code MATCHES "${bad_include}" OR code MATCHES "${bad_call}")
      file(RELATIVE_PATH rel "${SRC}" "${f}")
      string(STRIP "${code}" code)
      list(APPEND problems "${rel}:${n}: ${code}")
    endif()
  endforeach()
endforeach()

if(problems)
  list(JOIN problems "\n  " text)
  message(FATAL_ERROR "world/ reaches the UI directly; use GetWorldLink () instead:\n  ${text}")
endif()
message(STATUS "world/ layering: ok")
