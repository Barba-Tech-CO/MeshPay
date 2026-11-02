/// DB passwords travel inside connection URLs without escaping, so they must be hex.
pub fn ensure_hex_password(name: &str, value: &str) -> Result<(), String> {
  if value.len() < 16 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
    return Err(format!(
      "{name} must be at least 16 hex characters (no URL-special characters)"
    ));
  }
  Ok(())
}

/// The password of a `scheme://user:password@host/...` URL.
pub fn url_password(url: &str) -> Option<&str> {
  let authority = url.split_once("://")?.1.split('/').next()?;
  let credentials = authority.rsplit_once('@')?.0;
  Some(credentials.split_once(':')?.1)
}

pub fn ensure_hex_url_password(name: &str, url: &str) -> Result<(), String> {
  let password = url_password(url).ok_or_else(|| format!("{name} has no password"))?;
  ensure_hex_password(&format!("password in {name}"), password)
}
