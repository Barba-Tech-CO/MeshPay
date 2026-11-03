//! Canonical signing bytes: every field is length-prefixed (u32 BE), so fields cannot slide.
pub fn put(buf: &mut Vec<u8>, field: &[u8]) {
  buf.extend_from_slice(&(field.len() as u32).to_be_bytes());
  buf.extend_from_slice(field);
}

pub fn put_u64(buf: &mut Vec<u8>, value: u64) {
  put(buf, &value.to_be_bytes());
}
