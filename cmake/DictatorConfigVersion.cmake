# SPDX-License-Identifier: MIT OR Apache-2.0
#
# The version is the one `dictator --version` reports, asked only when
# find_package(Dictator <version>) wants one. Any newer version is accepted;
# a binary that is missing or reports no version is not.

if(NOT PACKAGE_FIND_VERSION)
  return()
endif()
find_program(DICTATOR_EXECUTABLE dictator)
execute_process(
  COMMAND "${DICTATOR_EXECUTABLE}" --version
  OUTPUT_VARIABLE version
  ERROR_QUIET)
if(version MATCHES "^dictator ([0-9.]+)")
  set(PACKAGE_VERSION "${CMAKE_MATCH_1}")
endif()

if(PACKAGE_VERSION VERSION_LESS PACKAGE_FIND_VERSION
   OR (PACKAGE_FIND_VERSION_RANGE_MAX STREQUAL "INCLUDE" AND PACKAGE_VERSION VERSION_GREATER PACKAGE_FIND_VERSION_MAX)
   OR (PACKAGE_FIND_VERSION_RANGE_MAX STREQUAL "EXCLUDE" AND PACKAGE_VERSION VERSION_GREATER_EQUAL PACKAGE_FIND_VERSION_MAX))
  set(PACKAGE_VERSION_COMPATIBLE FALSE)
else()
  set(PACKAGE_VERSION_COMPATIBLE TRUE)
  if(PACKAGE_VERSION VERSION_EQUAL PACKAGE_FIND_VERSION)
    set(PACKAGE_VERSION_EXACT TRUE)
  endif()
endif()
