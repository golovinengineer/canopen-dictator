# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Generate a CANopen object dictionary with Dictator at build time.
# Needs CMake 3.20 or newer.
#
#   dictator_add_object_dictionary(<target>
#       PROJECT  <file>          # .objdict (or .objdict.toml), .xdd or .od
#       STACK    canfestival
#       NAME     <name>          # <name>.c / <name>.h
#       [LINK    <targets>...]   # the stack's headers, linked PUBLIC
#       [EDS     <file>]         # these two are built by <target>_docs;
#       [MARKDOWN <file>])       # relative paths are in the current build directory
#
# The tool is DICTATOR_EXECUTABLE if set, else `dictator` on PATH.

include_guard(GLOBAL)

find_program(DICTATOR_EXECUTABLE dictator)

function(dictator_add_object_dictionary target)
  cmake_parse_arguments(PARSE_ARGV 1 arg "" "PROJECT;STACK;NAME;EDS;MARKDOWN" "LINK")
  if(arg_UNPARSED_ARGUMENTS)
    message(FATAL_ERROR "dictator_add_object_dictionary: unknown arguments ${arg_UNPARSED_ARGUMENTS}")
  endif()
  foreach(required PROJECT STACK NAME)
    if(NOT arg_${required})
      message(FATAL_ERROR "dictator_add_object_dictionary: ${required} is required")
    endif()
  endforeach()
  if(NOT arg_STACK STREQUAL "canfestival")
    message(FATAL_ERROR "dictator_add_object_dictionary: unknown STACK ${arg_STACK} (supported: canfestival)")
  endif()
  if(NOT EXISTS "${DICTATOR_EXECUTABLE}")
    message(FATAL_ERROR "dictator not found: put it on PATH or set DICTATOR_EXECUTABLE")
  endif()

  get_filename_component(project "${arg_PROJECT}" ABSOLUTE)
  set(dir "${CMAKE_CURRENT_BINARY_DIR}/${target}")
  add_custom_command(
    OUTPUT "${dir}/${arg_NAME}.c" "${dir}/${arg_NAME}.h"
    COMMAND "${DICTATOR_EXECUTABLE}" export "${project}" --c "${dir}/${arg_NAME}.c" --strict
    DEPENDS "${project}" "${DICTATOR_EXECUTABLE}"
    COMMENT "Generating ${arg_NAME}.c from ${arg_PROJECT}"
    VERBATIM)
  add_library(${target} STATIC "${dir}/${arg_NAME}.c")
  target_include_directories(${target} PUBLIC "${dir}")
  target_link_libraries(${target} PUBLIC ${arg_LINK})

  set(docs)
  set(docs_args)
  if(arg_EDS)
    list(APPEND docs "${arg_EDS}")
    list(APPEND docs_args --eds "${arg_EDS}")
  endif()
  if(arg_MARKDOWN)
    list(APPEND docs "${arg_MARKDOWN}")
    list(APPEND docs_args --md "${arg_MARKDOWN}")
  endif()
  if(docs)
    add_custom_command(
      OUTPUT ${docs}
      COMMAND "${DICTATOR_EXECUTABLE}" export "${project}" ${docs_args} --strict
      DEPENDS "${project}" "${DICTATOR_EXECUTABLE}"
      COMMENT "Generating ${target} docs from ${arg_PROJECT}"
      VERBATIM)
    add_custom_target(${target}_docs DEPENDS ${docs})
  endif()
endfunction()
