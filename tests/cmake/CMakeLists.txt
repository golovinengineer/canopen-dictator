# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Builds the example's object dictionary with cmake/Dictator.cmake; run by
# run.sh. The example .objdict.toml keeps a UNICODE_STRING object that
# CanFestival can't export, so this uses the example's CanFestival .od.

cmake_minimum_required(VERSION 3.20...4.5)
project(dictator_cmake_test LANGUAGES C)

# The CanFestival stack the generated C is written for (CONSTSTORE,
# DEVICE_TYPE_SLAVE), from co_objdictgen's authors. No licence file, so fetched,
# not vendored. Only its headers are used: include/ has no CMakeLists.txt, so
# the stack itself is not built.
include(FetchContent)
FetchContent_Declare(co_stack
  URL https://github.com/RoboticsHardwareSolutions/co_stack/archive/0e1892cbd9dd17ce34a164c58b4c59458bface79.tar.gz
  URL_HASH SHA256=cd31212b9acef34e9fac10d7dc663c00ca430a9fbce45d1a0f85e51adcf16552
  SOURCE_SUBDIR include)
FetchContent_MakeAvailable(co_stack)
add_library(canfestival INTERFACE)
target_include_directories(canfestival INTERFACE "${co_stack_SOURCE_DIR}/include")

find_package(Dictator 0.2 REQUIRED)
dictator_add_object_dictionary(od
  PROJECT  "${CMAKE_CURRENT_SOURCE_DIR}/../../assets/example/example.od"
  STACK    canfestival
  NAME     ObjDict
  LINK     canfestival
  EDS      example.eds
  MARKDOWN example.md)

add_library(firmware OBJECT firmware.c)
target_link_libraries(firmware PRIVATE od)
