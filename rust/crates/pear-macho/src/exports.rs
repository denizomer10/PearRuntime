//! The exports trie (LC_DYLD_EXPORTS_TRIE or dyld-info's export area).

use crate::image::{c_str, Error, Export, ExportKind};

const EXPORT_SYMBOL_FLAGS_KIND_MASK: u64 = 0x03;
const EXPORT_SYMBOL_FLAGS_KIND_THREAD_LOCAL: u64 = 0x01;
const EXPORT_SYMBOL_FLAGS_KIND_ABSOLUTE: u64 = 0x02;
const EXPORT_SYMBOL_FLAGS_WEAK_DEFINITION: u64 = 0x04;
const EXPORT_SYMBOL_FLAGS_REEXPORT: u64 = 0x08;
const EXPORT_SYMBOL_FLAGS_STUB_AND_RESOLVER: u64 = 0x10;

pub(crate) fn uleb(d: &[u8], pos: &mut usize) -> Result<u64, Error> {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let b = *d.get(*pos).ok_or(Error::Truncated("uleb128"))?;
        *pos += 1;
        if shift < 64 {
            v |= ((b & 0x7f) as u64) << shift;
        }
        if b & 0x80 == 0 {
            return Ok(v);
        }
        shift += 7;
        if shift > 70 {
            return Err(Error::Malformed("uleb128"));
        }
    }
}

pub(crate) fn sleb(d: &[u8], pos: &mut usize) -> Result<i64, Error> {
    let mut v = 0i64;
    let mut shift = 0;
    loop {
        let b = *d.get(*pos).ok_or(Error::Truncated("sleb128"))?;
        *pos += 1;
        if shift < 64 {
            v |= ((b & 0x7f) as i64) << shift;
        }
        shift += 7;
        if b & 0x80 == 0 {
            if shift < 64 && b & 0x40 != 0 {
                v |= -1i64 << shift;
            }
            return Ok(v);
        }
        if shift > 70 {
            return Err(Error::Malformed("sleb128"));
        }
    }
}

pub(crate) fn walk(trie: &[u8]) -> Result<Vec<Export>, Error> {
    let mut out = Vec::new();
    let mut stack = vec![(0usize, String::new())];
    let mut visited = 0usize;
    while let Some((node, prefix)) = stack.pop() {
        visited += 1;
        if visited > trie.len() + 1 {
            return Err(Error::Malformed("export trie loop"));
        }
        let mut pos = node;
        let terminal = uleb(trie, &mut pos)? as usize;
        let children_at = pos + terminal;
        if terminal > 0 {
            let flags = uleb(trie, &mut pos)?;
            let kind = if flags & EXPORT_SYMBOL_FLAGS_REEXPORT != 0 {
                let ordinal = uleb(trie, &mut pos)? as usize;
                let name = c_str(trie, pos)?;
                ExportKind::Reexport { dylib: ordinal.checked_sub(1).ok_or(Error::Malformed("re-export ordinal"))?, name }
            } else if flags & EXPORT_SYMBOL_FLAGS_STUB_AND_RESOLVER != 0 {
                let stub = uleb(trie, &mut pos)?;
                let resolver = uleb(trie, &mut pos)?;
                ExportKind::Resolver { stub, resolver }
            } else if flags & EXPORT_SYMBOL_FLAGS_KIND_MASK == EXPORT_SYMBOL_FLAGS_KIND_ABSOLUTE {
                ExportKind::Absolute { value: uleb(trie, &mut pos)? }
            } else {
                ExportKind::Regular {
                    offset: uleb(trie, &mut pos)?,
                    weak: flags & EXPORT_SYMBOL_FLAGS_WEAK_DEFINITION != 0,
                    thread_local: flags & EXPORT_SYMBOL_FLAGS_KIND_MASK == EXPORT_SYMBOL_FLAGS_KIND_THREAD_LOCAL,
                }
            };
            out.push(Export { name: prefix.clone(), kind });
        }
        let mut pos = children_at;
        let count = *trie.get(pos).ok_or(Error::Truncated("export trie"))?;
        pos += 1;
        for _ in 0..count {
            let edge = c_str(trie, pos)?;
            pos += edge.len() + 1;
            let child = uleb(trie, &mut pos)? as usize;
            if child >= trie.len() {
                return Err(Error::Malformed("export trie child"));
            }
            stack.push((child, format!("{prefix}{edge}")));
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
