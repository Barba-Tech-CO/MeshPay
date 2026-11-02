use meshpay_core::api::{NoiseSession, generate_noise_static_keypair};
use snow::{Builder, params::NoiseParams};

const PROLOGUE: &[u8] =
  b"meshpay://charge?v=1&sid=7f3a9c2e41d84b6a&amt=1250&cur=BRL&exp=1767225600";
const VECTORS: &str = include_str!("vectors/cacophony_noise_xk_25519_chachapoly_blake2s.json");

fn hex(value: &str) -> Vec<u8> {
  (0..value.len())
    .step_by(2)
    .map(|index| u8::from_str_radix(&value[index..index + 2], 16).expect("valid hex"))
    .collect()
}

fn sessions(prologue: &[u8], responder_prologue: &[u8]) -> (NoiseSession, NoiseSession) {
  let responder_keys = generate_noise_static_keypair().expect("keypair");
  let initiator =
    NoiseSession::new_initiator(responder_keys.public_key, prologue.to_vec()).expect("initiator");
  let responder =
    NoiseSession::new_responder(responder_keys.private_key, responder_prologue.to_vec())
      .expect("responder");

  (initiator, responder)
}

fn handshake(
  initiator: &mut NoiseSession,
  responder: &mut NoiseSession,
) -> Result<[usize; 3], String> {
  let first = initiator.write_message(vec![])?;
  responder.read_message(first.clone())?;
  let second = responder.write_message(vec![])?;
  initiator.read_message(second.clone())?;
  let third = initiator.write_message(vec![])?;
  responder.read_message(third.clone())?;

  Ok([first.len(), second.len(), third.len()])
}

#[test]
fn official_cacophony_vectors_replay_byte_for_byte() {
  let file: serde_json::Value = serde_json::from_str(VECTORS).expect("vectors json");
  let vectors = file["vectors"].as_array().expect("vectors array");
  assert_eq!(vectors.len(), 1);
  let params: NoiseParams = "Noise_XK_25519_ChaChaPoly_BLAKE2s".parse().expect("params");

  for vector in vectors {
    let field = |name: &str| hex(vector[name].as_str().expect(name));
    let (init_static, init_remote, init_ephemeral) = (
      field("init_static"),
      field("init_remote_static"),
      field("init_ephemeral"),
    );
    let (resp_static, resp_ephemeral) = (field("resp_static"), field("resp_ephemeral"));
    let (init_prologue, resp_prologue) = (field("init_prologue"), field("resp_prologue"));
    let mut initiator = Builder::new(params.clone())
      .fixed_ephemeral_key_for_testing_only(&init_ephemeral)
      .local_private_key(&init_static)
      .unwrap()
      .remote_public_key(&init_remote)
      .unwrap()
      .prologue(&init_prologue)
      .unwrap()
      .build_initiator()
      .unwrap();
    let mut responder = Builder::new(params.clone())
      .fixed_ephemeral_key_for_testing_only(&resp_ephemeral)
      .local_private_key(&resp_static)
      .unwrap()
      .prologue(&resp_prologue)
      .unwrap()
      .build_responder()
      .unwrap();
    let messages = vector["messages"].as_array().expect("messages");
    let mut buffer = vec![0; 65535];
    let mut plain = vec![0; 65535];

    for (index, message) in messages.iter().take(3).enumerate() {
      let (writer, reader) = if index % 2 == 0 {
        (&mut initiator, &mut responder)
      } else {
        (&mut responder, &mut initiator)
      };
      let payload = hex(message["payload"].as_str().unwrap());
      let length = writer.write_message(&payload, &mut buffer).unwrap();
      assert_eq!(
        buffer[..length],
        hex(message["ciphertext"].as_str().unwrap())[..],
        "message {index}"
      );
      let plain_length = reader.read_message(&buffer[..length], &mut plain).unwrap();
      assert_eq!(plain[..plain_length], payload[..]);
    }

    assert!(initiator.is_handshake_finished() && responder.is_handshake_finished());
    let expected_hash = hex(vector["handshake_hash"].as_str().unwrap());
    assert_eq!(initiator.get_handshake_hash(), &expected_hash[..]);
    assert_eq!(responder.get_handshake_hash(), &expected_hash[..]);
    let mut initiator = initiator.into_transport_mode().unwrap();
    let mut responder = responder.into_transport_mode().unwrap();

    for (index, message) in messages.iter().enumerate().skip(3) {
      let (writer, reader) = if index % 2 == 0 {
        (&mut initiator, &mut responder)
      } else {
        (&mut responder, &mut initiator)
      };
      let payload = hex(message["payload"].as_str().unwrap());
      let length = writer.write_message(&payload, &mut buffer).unwrap();
      assert_eq!(
        buffer[..length],
        hex(message["ciphertext"].as_str().unwrap())[..],
        "message {index}"
      );
      let plain_length = reader.read_message(&buffer[..length], &mut plain).unwrap();
      assert_eq!(plain[..plain_length], payload[..]);
    }
  }
}

#[test]
fn handshake_messages_with_empty_payload_have_expected_sizes() {
  let (mut initiator, mut responder) = sessions(PROLOGUE, PROLOGUE);

  let sizes = handshake(&mut initiator, &mut responder).expect("handshake");

  assert_eq!(sizes, [48, 48, 64]);
  assert_eq!(sizes.iter().sum::<usize>(), 160);
  assert!(initiator.is_finished() && responder.is_finished());
}

#[test]
fn flipping_any_prologue_byte_on_responder_fails_handshake() {
  let mut failures = 0;

  for position in 0..PROLOGUE.len() {
    let mut tampered = PROLOGUE.to_vec();
    tampered[position] ^= 0x01;
    let (mut initiator, mut responder) = sessions(PROLOGUE, &tampered);

    if handshake(&mut initiator, &mut responder).is_err() {
      failures += 1;
    }
    assert!(!responder.is_finished(), "byte {position}");
  }

  assert_eq!(failures, PROLOGUE.len());
}

#[test]
fn third_party_without_responder_private_key_cannot_complete() {
  let real = generate_noise_static_keypair().expect("keypair");
  let impostor = generate_noise_static_keypair().expect("keypair");
  let mut initiator =
    NoiseSession::new_initiator(real.public_key, PROLOGUE.to_vec()).expect("initiator");
  let mut impostor_session =
    NoiseSession::new_responder(impostor.private_key, PROLOGUE.to_vec()).expect("impostor");

  let first = initiator.write_message(vec![]).expect("first message");

  assert!(impostor_session.read_message(first).is_err());
  assert!(!impostor_session.is_finished());
  assert!(impostor_session.write_message(vec![]).is_err());
  assert!(!initiator.is_finished());
}

#[test]
fn transport_round_trips_in_both_directions() {
  let (mut initiator, mut responder) = sessions(PROLOGUE, PROLOGUE);
  handshake(&mut initiator, &mut responder).expect("handshake");

  let forward = initiator.encrypt(b"tx_propose".to_vec()).expect("encrypt");
  assert_eq!(responder.decrypt(forward).expect("decrypt"), b"tx_propose");
  let backward = responder
    .encrypt(b"tx_commit_ack".to_vec())
    .expect("encrypt");
  assert_eq!(
    initiator.decrypt(backward).expect("decrypt"),
    b"tx_commit_ack"
  );
  assert!(
    initiator
      .decrypt(b"garbage garbage garbage".to_vec())
      .is_err()
  );
}

#[test]
fn transport_is_unavailable_before_handshake_finishes() {
  let (mut initiator, _) = sessions(PROLOGUE, PROLOGUE);

  assert!(initiator.encrypt(b"early".to_vec()).is_err());
}

#[test]
fn generated_static_keypairs_all_differ() {
  let keypairs: Vec<_> = (0..3)
    .map(|_| generate_noise_static_keypair().expect("keypair"))
    .collect();

  for (index, first) in keypairs.iter().enumerate() {
    assert_eq!(first.public_key.len(), 32);
    assert_eq!(first.private_key.len(), 32);
    for second in &keypairs[index + 1..] {
      assert_ne!(first.public_key, second.public_key);
      assert_ne!(first.private_key, second.private_key);
    }
  }
}
