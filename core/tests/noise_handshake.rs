use snow::{Builder, params::NoiseParams};

#[test]
fn noise_xk_handshake_completes_in_process() -> Result<(), snow::Error> {
  let params: NoiseParams = "Noise_XK_25519_ChaChaPoly_BLAKE2s".parse()?;
  let initiator_keypair = Builder::new(params.clone()).generate_keypair()?;
  let responder_keypair = Builder::new(params.clone()).generate_keypair()?;
  let mut initiator = Builder::new(params.clone())
    .local_private_key(&initiator_keypair.private)?
    .remote_public_key(&responder_keypair.public)?
    .build_initiator()?;
  let mut responder = Builder::new(params)
    .local_private_key(&responder_keypair.private)?
    .build_responder()?;
  let mut message = vec![0; 1024];
  let mut payload = vec![0; 1024];

  let length = initiator.write_message(b"1", &mut message)?;
  let payload_length = responder.read_message(&message[..length], &mut payload)?;
  assert_eq!(&payload[..payload_length], b"1");
  let length = responder.write_message(b"2", &mut message)?;
  let payload_length = initiator.read_message(&message[..length], &mut payload)?;
  assert_eq!(&payload[..payload_length], b"2");
  let length = initiator.write_message(b"3", &mut message)?;
  let payload_length = responder.read_message(&message[..length], &mut payload)?;
  assert_eq!(&payload[..payload_length], b"3");

  Ok(())
}
