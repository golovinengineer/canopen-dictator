#!/bin/sh
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Build this project with the given dictator binary, then check that a
# rebuild with nothing changed doesn't regenerate the object dictionary and
# that touching the project file does. Then build from .objdict and
# .objdict.toml imports of the example.
#
#   tests/cmake/run.sh DICTATOR BUILD_DIR [cmake configure options...]
set -eu
dictator=$1
build=$2
shift 2
here=$(cd "$(dirname "$0")" && pwd)

cmake -S "$here" -B "$build" -DDictator_DIR="$here/../../cmake" -DDICTATOR_EXECUTABLE="$dictator" "$@"
cmake --build "$build"
cmake --build "$build" --target od_docs
test -s "$build/example.eds" && test -s "$build/example.md"

cmake --build "$build" >"$build/log"
if grep "Generating ObjDict.c" "$build/log"; then
  echo "FAIL: unchanged rebuild regenerated ObjDict.c" >&2
  exit 1
fi

# Coarse file timestamps: make the touched project file strictly newer.
sleep 1
touch "$here/../../assets/example/example.od"
cmake --build "$build" >"$build/log"
if ! grep "Generating ObjDict.c" "$build/log"; then
  echo "FAIL: touching the project file did not regenerate ObjDict.c" >&2
  exit 1
fi

for name in example.objdict example.objdict.toml; do
  "$dictator" import "$here/../../assets/example/example.od" "$build/$name"
  cmake -S "$here" -B "$build" -DOD_PROJECT="$build/$name"
  cmake --build "$build" >"$build/log"
  grep -q "Generating ObjDict.c from $build/$name" "$build/log"
done
echo "OK"
