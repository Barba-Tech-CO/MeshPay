//! Synthetic Android attestation PKI: test root -> intermediate -> leaf with a crafted KeyDescription.
use meshpay_server::attestation::key_description::{
  ALGORITHM_EC, EC_CURVE_P256, ORIGIN_GENERATED, PURPOSE_SIGN, SECURITY_LEVEL_SOFTWARE,
  SECURITY_LEVEL_STRONGBOX, SECURITY_LEVEL_TEE, VERIFIED_BOOT_VERIFIED,
};
use p256::ecdsa::VerifyingKey;
use p256::pkcs8::EncodePublicKey;
use rcgen::{
  BasicConstraints, Certificate, CertificateParams, CustomExtension, DnType, IsCa, Issuer, KeyPair,
  SerialNumber, SubjectPublicKeyInfo, date_time_ymd,
};
use time::OffsetDateTime;

fn length(n: usize) -> Vec<u8> {
  if n < 128 {
    return vec![n as u8];
  }
  let bytes: Vec<u8> = n
    .to_be_bytes()
    .into_iter()
    .skip_while(|b| *b == 0)
    .collect();
  [vec![0x80 | bytes.len() as u8], bytes].concat()
}

pub fn tlv(tag: &[u8], content: &[u8]) -> Vec<u8> {
  [tag, &length(content.len()), content].concat()
}

fn integer(value: u32) -> Vec<u8> {
  let mut bytes: Vec<u8> = value
    .to_be_bytes()
    .into_iter()
    .skip_while(|b| *b == 0)
    .collect();
  if bytes.first().is_none_or(|b| b & 0x80 != 0) {
    bytes.insert(0, 0);
  }
  tlv(&[0x02], &bytes)
}

fn enumerated(value: u32) -> Vec<u8> {
  let mut out = integer(value);
  out[0] = 0x0a;
  out
}

/// Context-specific constructed tag with a (possibly multi-byte) tag number, explicit.
fn explicit(number: u32, inner: &[u8]) -> Vec<u8> {
  let tag = if number < 31 {
    vec![0xa0 | number as u8]
  } else {
    let mut digits = vec![(number & 0x7f) as u8];
    let mut rest = number >> 7;
    while rest > 0 {
      digits.insert(0, 0x80 | (rest & 0x7f) as u8);
      rest >>= 7;
    }
    [vec![0xbf], digits].concat()
  };
  tlv(&tag, inner)
}

#[derive(Clone)]
pub struct KeySpec {
  pub attestation_level: u32,
  pub keymaster_level: u32,
  pub challenge: Vec<u8>,
  pub purposes: Vec<u32>,
  pub algorithm: u32,
  pub curve: u32,
  pub origin: u32,
  /// (deviceLocked, verifiedBootState); only written to the tee-enforced list.
  pub boot: Option<(bool, u32)>,
  /// Tag byte of the purposes field; the real one is context-specific constructed 1 (0xa1).
  pub purposes_tag: u8,
}

impl KeySpec {
  fn base(level: u32, challenge: &[u8]) -> Self {
    Self {
      attestation_level: level,
      keymaster_level: level,
      challenge: challenge.to_vec(),
      purposes: vec![PURPOSE_SIGN],
      algorithm: ALGORITHM_EC,
      curve: EC_CURVE_P256,
      origin: ORIGIN_GENERATED,
      boot: (level > SECURITY_LEVEL_SOFTWARE).then_some((true, VERIFIED_BOOT_VERIFIED)),
      purposes_tag: 0xa1,
    }
  }

  pub fn strongbox(challenge: &[u8]) -> Self {
    Self::base(SECURITY_LEVEL_STRONGBOX, challenge)
  }

  pub fn tee(challenge: &[u8]) -> Self {
    Self::base(SECURITY_LEVEL_TEE, challenge)
  }

  pub fn software(challenge: &[u8]) -> Self {
    Self::base(SECURITY_LEVEL_SOFTWARE, challenge)
  }

  fn authorization_list(&self) -> Vec<u8> {
    let purposes: Vec<u8> = self.purposes.iter().flat_map(|p| integer(*p)).collect();
    let mut fields = [
      tlv(&[self.purposes_tag], &tlv(&[0x31], &purposes)),
      explicit(2, &integer(self.algorithm)),
      explicit(10, &integer(self.curve)),
    ]
    .concat();
    fields.extend(explicit(702, &integer(self.origin)));
    if let (Some((locked, state)), true) = (self.boot, self.attestation_level > 0) {
      let root = [
        tlv(&[0x04], &[7u8; 32]),
        tlv(&[0x01], &[if locked { 0xff } else { 0x00 }]),
        enumerated(state),
        tlv(&[0x04], &[9u8; 32]),
      ]
      .concat();
      fields.extend(explicit(704, &tlv(&[0x30], &root)));
    }
    tlv(&[0x30], &fields)
  }

  pub fn der(&self) -> Vec<u8> {
    let (software, tee) = if self.attestation_level > 0 {
      (tlv(&[0x30], &[]), self.authorization_list())
    } else {
      (self.authorization_list(), tlv(&[0x30], &[]))
    };
    let body = [
      integer(4),
      enumerated(self.attestation_level),
      integer(4),
      enumerated(self.keymaster_level),
      tlv(&[0x04], &self.challenge),
      tlv(&[0x04], &[]),
      software,
      tee,
    ]
    .concat();
    tlv(&[0x30], &body)
  }
}

pub struct Pki {
  pub root_der: Vec<u8>,
  intermediate_der: Vec<u8>,
  intermediate: Issuer<'static, KeyPair>,
}

fn ca_params(name: &str) -> CertificateParams {
  let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
  params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
  params.distinguished_name.push(DnType::CommonName, name);
  params.not_before = date_time_ymd(2020, 1, 1);
  params.not_after = date_time_ymd(2099, 1, 1);
  params
}

impl Pki {
  pub fn new() -> Self {
    let root_key = KeyPair::generate().unwrap();
    let root_params = ca_params("test root");
    let root: Certificate = root_params.self_signed(&root_key).unwrap();
    let root_issuer = Issuer::new(root_params, root_key);
    let intermediate_key = KeyPair::generate().unwrap();
    let intermediate_params = ca_params("test intermediate");
    let intermediate = intermediate_params
      .signed_by(&intermediate_key, &root_issuer)
      .unwrap();
    Self {
      root_der: root.der().to_vec(),
      intermediate_der: intermediate.der().to_vec(),
      intermediate: Issuer::new(intermediate_params, intermediate_key),
    }
  }

  /// `[leaf, intermediate, root]`. `extension` is the raw KeyDescription DER, or none.
  pub fn chain(
    &self,
    leaf_key: &VerifyingKey,
    extension: Option<Vec<u8>>,
    serial: u64,
    not_after: OffsetDateTime,
  ) -> Vec<Vec<u8>> {
    let spki =
      SubjectPublicKeyInfo::from_der(leaf_key.to_public_key_der().unwrap().as_bytes()).unwrap();
    self.chain_for_spki(&spki, extension, serial, not_after)
  }

  /// Same as `chain` for any leaf key type (used for non-P-256 leaves).
  pub fn chain_for_spki(
    &self,
    spki: &SubjectPublicKeyInfo,
    extension: Option<Vec<u8>>,
    serial: u64,
    not_after: OffsetDateTime,
  ) -> Vec<Vec<u8>> {
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params
      .distinguished_name
      .push(DnType::CommonName, "test leaf");
    params.not_before = date_time_ymd(2020, 1, 1);
    params.not_after = not_after;
    params.serial_number = Some(SerialNumber::from(serial));
    if let Some(der) = extension {
      params
        .custom_extensions
        .push(CustomExtension::from_oid_content(
          &[1, 3, 6, 1, 4, 1, 11129, 2, 1, 17],
          der,
        ));
    }
    let leaf = params.signed_by(spki, &self.intermediate).unwrap();
    vec![
      leaf.der().to_vec(),
      self.intermediate_der.clone(),
      self.root_der.clone(),
    ]
  }

  pub fn valid_chain(&self, leaf_key: &VerifyingKey, spec: &KeySpec) -> Vec<Vec<u8>> {
    self.chain(leaf_key, Some(spec.der()), 4242, date_time_ymd(2098, 1, 1))
  }
}
