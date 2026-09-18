"""Dumps what the spike drives from a face GLB: faceId, rootBounds, RobotData
node names/ids, and the rig graph's input paths matching a pattern."""
import json, struct, sys, re
path, pat = sys.argv[1], (sys.argv[2] if len(sys.argv) > 2 else r"eye|jaw|mouth")
b = open(path, 'rb').read()
n = struct.unpack_from('<I', b, 12)[0]
g = json.loads(b[20:20 + n])
bundle = next((nd['extensions']['VIZIJ_bundle'] for nd in g['nodes'] if 'VIZIJ_bundle' in nd.get('extensions', {})), g.get('extensions', {}).get('VIZIJ_bundle'))
print('faceId', bundle['metadata'].get('faceId'), 'graphs', [(x['kind'], x.get('id')) for x in bundle['graphs']])
for nd in g['nodes']:
    rd = nd.get('extensions', {}).get('RobotData')
    if rd:
        print('node', nd.get('name'), 'id', rd.get('id'), 'shape', rd.get('shape'), 'rootBounds', rd.get('rootBounds'), 'morphs', rd.get('morphTargets'))
rig = next(x['spec'] for x in bundle['graphs'] if x['kind'] == 'rig')
nodes = rig['nodes'] if isinstance(rig['nodes'], list) else list(rig['nodes'].values())
for nd in nodes:
    if nd.get('type') == 'input':
        p = nd.get('params', {}).get('path')
        if p and re.search(pat, p, re.I) and '/override/' not in p:
            print('input', p, nd.get('params', {}).get('value'))
