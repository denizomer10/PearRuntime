#!/usr/bin/env python3
"""pear-macho's fixups against Apple's `dyld_info -fixups`, for every arm64
Mach-O under a directory (apps, frameworks, dylibs).

  cargo build --release -p pear-macho --example fixups-dump
  tools/macho-fixups-compare.py rust/target/release/examples/fixups-dump DIR

Weak-definition coalescing entries are matched by address only (dyld_info
labels them unreliably); files dyld_info itself fails on are skipped and
listed. Prints a summary and the files that differ.
"""
import os, subprocess, sys, collections, re
dump = sys.argv[1]; root = sys.argv[2].rstrip("/")
def macho(p):
    try:
        with open(p,'rb') as f: m=f.read(4)
        return m in (b'\xcf\xfa\xed\xfe', b'\xca\xfe\xba\xbe')
    except: return False
files=[]
for r,ds,fs in os.walk(root):
    if '_CodeSignature' in r: continue
    for n in fs:
        p=os.path.join(r,n)
        if not os.path.islink(p) and os.path.isfile(p) and macho(p): files.append(p)
stats=collections.Counter(); bad=[]
for p in files:
    run=subprocess.run(['xcrun','dyld_info','-arch','arm64','-fixups',p],capture_output=True,text=True)
    out=run.stdout.splitlines()
    if run.returncode!=0:
        stats['dyld_info crashed (skipped)']+=1; bad.append((p,f'dyld_info exit {run.returncode}')); continue
    theirs=[]
    for l in out:
        m=re.match(r'\s+\S+\s+\S+\s+(0x[0-9A-F]+)\s+(\S+)\s+(.*)$',l)
        if m and m.group(1)!='address': theirs.append(f"{m.group(1)} {m.group(2)} {m.group(3).strip()}")
    ours=subprocess.run([dump,p],capture_output=True,text=True).stdout.splitlines()
    if ours and ours[0].startswith('error'):
        stats['ours error: '+ours[0]]+=1; bad.append((p,ours[0])); continue
    if not theirs and not ours: stats['no fixups (or no arm64)']+=1; continue
    # Weak-def coalescing entries: dyld_info labels them unreliably, so they
    # are matched by address only (one entry of theirs per weak entry of ours).
    weak_ours=[l for l in ours if '<weak-def-coalesce>' in l]
    ours=[l for l in ours if '<weak-def-coalesce>' not in l]
    by_addr=collections.defaultdict(list)
    for i,l in enumerate(theirs): by_addr[l.split(' ',1)[0]].append(i)
    drop=set()
    for w in weak_ours:
        idx=[i for i in by_addr.get(w.split()[0],[]) if i not in drop]
        pref=[i for i in idx if '<weak-def-coalesce>' in theirs[i]] or idx[::-1]
        if pref: drop.add(pref[0]); stats['weak matched by address']+=1
        else:
            stats['weak unmatched']+=1
            if stats['weak unmatched']<=3: print('weak unmatched:', p.replace(root+'/',''), w)
    theirs=[l for i,l in enumerate(theirs) if i not in drop]
    a=collections.Counter(theirs); b=collections.Counter(ours)
    if a==b: stats['identical']+=1
    else:
        stats['differ']+=1
        bad.append((p, f"{sum((a-b).values())} only dyld_info e.g. {list((a-b).elements())[:2]}; {sum((b-a).values())} only ours e.g. {list((b-a).elements())[:2]}"))
    stats['fixups']+=len(theirs)
print(dict(stats))
for p,why in bad[:15]: print(p.replace(root+'/',''), '::', why[:300])
