#!/usr/bin/env python3
"""Generate typed GLES ABI forwarders from ANGLE's pinned Khronos registry."""
import pathlib
import re
import sys
import xml.etree.ElementTree as ET

root = pathlib.Path(__file__).resolve().parent
registry = root / 'third_party/angle/third_party/OpenGL-Registry/src/xml/gl.xml'
tree = ET.parse(registry).getroot()
commands = {node.findtext('proto/name') or node.get('name'): node for node in tree.findall('commands/command')}
selected = set()
for feature in tree.findall('feature'):
    if feature.get('api') in ('gles1', 'gles2'):
        for requirement in feature.findall('require'):
            if requirement.get('api') in (None, 'gles1', 'gles2'):
                selected.update(node.get('name') for node in requirement.findall('command'))
for extension in tree.findall('extensions/extension'):
    if set(extension.get('supported', '').split('|')) & {'gles1', 'gles2'}:
        for requirement in extension.findall('require'):
            if requirement.get('api') in (None, 'gles1', 'gles2'):
                selected.update(node.get('name') for node in requirement.findall('command'))
# Only forward entry points exported by the pinned ANGLE build.
definitions = (root / 'third_party/angle/src/libGLESv2/libGLESv2_autogen.def').read_text()
exports = set(re.findall(r'^\s+(gl\w+)\s*$', definitions, re.M))
selected &= exports
lines = ['// Generated from the pinned Khronos registry (Apache-2.0) and ANGLE export list.',
         '// Copyright 2013-2026 The Khronos Group Inc.; original forwarding generator is PearRuntime.',
         '#include <GLES/gl.h>', '#include <GLES/glext.h>', '#include <GLES2/gl2.h>',
         '#include <GLES2/gl2ext.h>', '#include <GLES3/gl32.h>',
         '#include "PearANGLEGameRouting.h"', '#include "PearANGLEBackend.h"', '#include <cstdlib>',
         'static void *PearRequireGLES(const char *name) {',
         '    void *function = PearANGLEGameSymbol(name);',
         '    if (!function) { PearANGLETrace("[PearRuntime][angle] unavailable GLES entry: %s\\n", name); std::abort(); }',
         '    return function;', '}']
for name in sorted(selected):
    node = commands[name]
    if node.find('proto') is None:
        node = commands[node.get('alias')]
    original = node.findtext('proto/name')
    prototype = ''.join(node.find('proto').itertext()).strip()
    result = prototype[:prototype.rfind(original)].strip()
    params = [''.join(p.itertext()).strip() for p in node.findall('param')]
    args = [p.findtext('name') for p in node.findall('param')]
    signature = ', '.join(params) or 'void'
    lines += [f'extern "C" {result} GL_APIENTRY {name}({signature}) {{',
              f'    using Function = {result} (GL_APIENTRY *)({signature});',
              f'    static Function function = (Function)PearRequireGLES("{name}");',
              f'    return function({", ".join(args)});', '}']
pathlib.Path(sys.argv[1]).write_text('\n'.join(lines) + '\n')
print(f'Generated {len(selected)} typed GLES forwarders')
