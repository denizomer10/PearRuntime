//! Pure-Rust NIBArchive binary parser for PearRuntime.

#[derive(Clone, Debug, PartialEq)]
pub enum NibValuePayload {
    Int(i64),
    Double(f64),
    Bytes(Vec<u8>),
    Null,
    Ref(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NibValue {
    pub key: u32,
    pub value_type: u8,
    pub payload: NibValuePayload,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NibObject {
    pub cls: u32,
    pub first: u32,
    pub count: u32,
}

#[derive(Clone, Debug)]
pub struct NibArchive {
    pub keys: Vec<String>,
    pub classes: Vec<String>,
    pub objects: Vec<NibObject>,
    pub values: Vec<NibValue>,
}

fn read_varint(b: &[u8], i: &mut usize) -> Option<u32> {
    let mut v: u32 = 0;
    let mut shift = 0;
    while *i < b.len() && shift < 35 {
        let c = b[*i];
        *i += 1;
        v |= ((c & 0x7F) as u32) << shift;
        if (c & 0x80) != 0 {
            return Some(v);
        }
        shift += 7;
    }
    None
}

fn read_u32_le(b: &[u8], i: usize) -> Option<u32> {
    let s: [u8; 4] = b.get(i..i + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(s))
}

impl NibArchive {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 50 || &data[0..10] != b"NIBArchive" {
            return None;
        }
        let mut h = [0u32; 8];
        for (idx, slot) in h.iter_mut().enumerate() {
            *slot = read_u32_le(data, 18 + idx * 4)?;
        }

        let mut keys = Vec::with_capacity(h[2] as usize);
        let mut i = h[3] as usize;
        for _ in 0..h[2] {
            let n = read_varint(data, &mut i)? as usize;
            let slice = data.get(i..i + n)?;
            keys.push(String::from_utf8_lossy(slice).into_owned());
            i += n;
        }

        let mut classes = Vec::with_capacity(h[6] as usize);
        i = h[7] as usize;
        for _ in 0..h[6] {
            let n = read_varint(data, &mut i)? as usize;
            let extra = read_varint(data, &mut i)? as usize;
            i = i.checked_add(4 * extra)?;
            let slice = data.get(i..i + n)?;
            let mut end = n;
            while end > 0 && slice[end - 1] == 0 {
                end -= 1;
            }
            classes.push(String::from_utf8_lossy(&slice[..end]).into_owned());
            i += n;
        }

        let mut values = Vec::with_capacity(h[4] as usize);
        i = h[5] as usize;
        for _ in 0..h[4] {
            let key = read_varint(data, &mut i)?;
            let value_type = *data.get(i)?;
            i += 1;
            let payload = match value_type {
                0 => {
                    let b = *data.get(i)? as i8;
                    i += 1;
                    NibValuePayload::Int(b as i64)
                }
                1 => {
                    let s: [u8; 2] = data.get(i..i + 2)?.try_into().ok()?;
                    i += 2;
                    NibValuePayload::Int(i16::from_le_bytes(s) as i64)
                }
                2 => {
                    let s: [u8; 4] = data.get(i..i + 4)?.try_into().ok()?;
                    i += 4;
                    NibValuePayload::Int(i32::from_le_bytes(s) as i64)
                }
                3 => {
                    let s: [u8; 8] = data.get(i..i + 8)?.try_into().ok()?;
                    i += 8;
                    NibValuePayload::Int(i64::from_le_bytes(s))
                }
                4 => NibValuePayload::Int(1),
                5 => NibValuePayload::Int(0),
                6 => {
                    let s: [u8; 4] = data.get(i..i + 4)?.try_into().ok()?;
                    i += 4;
                    NibValuePayload::Double(f32::from_le_bytes(s) as f64)
                }
                7 => {
                    let s: [u8; 8] = data.get(i..i + 8)?.try_into().ok()?;
                    i += 8;
                    NibValuePayload::Double(f64::from_le_bytes(s))
                }
                8 => {
                    let n = read_varint(data, &mut i)? as usize;
                    let slice = data.get(i..i + n)?.to_vec();
                    i += n;
                    NibValuePayload::Bytes(slice)
                }
                9 => NibValuePayload::Null,
                10 => {
                    let s: [u8; 4] = data.get(i..i + 4)?.try_into().ok()?;
                    i += 4;
                    NibValuePayload::Ref(u32::from_le_bytes(s))
                }
                _ => return None,
            };
            values.push(NibValue {
                key,
                value_type,
                payload,
            });
        }

        let mut objects = Vec::with_capacity(h[0] as usize);
        i = h[1] as usize;
        for _ in 0..h[0] {
            let cls = read_varint(data, &mut i)?;
            let first = read_varint(data, &mut i)?;
            let count = read_varint(data, &mut i)?;
            if (cls as usize) >= classes.len() || first.saturating_add(count) > h[4] {
                return None;
            }
            objects.push(NibObject { cls, first, count });
        }

        Some(NibArchive {
            keys,
            classes,
            objects,
            values,
        })
    }

    pub fn value_for_key(&self, object_index: u32, key: &str) -> Option<&NibValue> {
        let obj = self.objects.get(object_index as usize)?;
        let start = obj.first as usize;
        let end = start + obj.count as usize;
        for v in self.values.get(start..end)? {
            if self.keys.get(v.key as usize).map(|s| s.as_str()) == Some(key) {
                return Some(v);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_magic() {
        assert!(NibArchive::parse(b"NOT_A_NIB").is_none());
    }
}
