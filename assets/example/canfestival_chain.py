# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Make the CanFestival references of the example set the way the old tool
# chain did: EDS -> objdictedit import -> CanFestival settings -> .od ->
# gen_cfile. Python 2; runs inside the objdictgen container
# (tools/objdictgen/Dockerfile), which mounts the repository at /workdir:
#
#   container exec co_stack_gen python2.7 /workdir/assets/example/canfestival_chain.py \
#       /workdir/assets/example/example.eds \
#       /workdir/assets/example/example.od \
#       /workdir/assets/example/ObjDict.c
import sys, os
os.chdir("/co_objdictgen-main"); sys.path.insert(0, "/co_objdictgen-main")
import __builtin__; __builtin__.__dict__['_'] = lambda s: s
from nodemanager import NodeManager
import gen_cfile
eds, od, c = sys.argv[1:4]
devnull = open(os.devnull, "w"); real = sys.stdout; sys.stdout = devnull  # eds_utils prints a lot
m = NodeManager()
r = m.ImportCurrentFromEDSFile(eds)
sys.stdout = real
if isinstance(r, (str, unicode)):
    print "IMPORT FAILED:", r; sys.exit(1)
n = m.CurrentNode
# CanFestival-only settings, as a user would set them in objdictedit.
n.SetNodeName("ExampleDevice")
# CANopenEditor writes the octet string default as "01020304" in the EDS and
# CanFestival's EDS parser reads that as an octal number (-> "270532"); set
# it back to the text, as a user would in objdictedit.
n.SetEntry(0x200C, 0, "01 02 03 04")
# Left out on the CanFestival side: UNICODE_STRING is not a CanFestival type,
# and gen_cfile refuses a mapped DOMAIN without an initial value.
for index in (0x200D, 0x200E):
    n.RemoveEntry(index)
    n.RemoveMappingEntry(index)
for index in (0x2000, 0x2006, 0x2101, 0x2102, 0x6200):
    n.SetParamsEntry(index, callback=True)
n.SetParamsEntry(0x2012, save=True)
for sub in (1, 2, 3):
    n.SetParamsEntry(0x2102, sub, save=True)
m.SaveCurrentInFile(od)
print "saved", od, len(n.Dictionary), "entries,", len(n.UserMapping), "user mappings"
print "gen_cfile:", gen_cfile.GenerateFile(c, n)
