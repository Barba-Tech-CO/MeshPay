//! In-process checks mapped to PRD v3 PV-REC-01..05. See docs/estudo/06c for what is not checkable.
use blahaj::Sharks;
use p256::ecdsa::{SigningKey, VerifyingKey};
use social_recovery::cheque::Cheque;
use social_recovery::device::Device;
use social_recovery::error::RecoveryError;
use social_recovery::keys::{random_signing_key, sign, verify};
use social_recovery::ledger::Ledger;
use social_recovery::params::{CANCEL_BLOCK, DAY, GUARDIANS, HOUR, RATE_WINDOW, T_REC};
use social_recovery::recovery_cancel::RecoveryCancel;
use social_recovery::recovery_cert::RecoveryCert;
use social_recovery::sealing::{SealedShare, open, seal};
use social_recovery::shares;

const ACCOUNT: &str = "acct-1";
const BALANCE: u64 = 1_000;
const FAR: u64 = 10_000 * DAY;

struct World {
  rk_public: VerifyingKey,
  owner: Device,
  shop: Device,
  guardian_keys: Vec<SigningKey>,
  sealed_shares: Vec<SealedShare>,
  opened: Vec<Vec<u8>>,
  ledger: Ledger,
}

fn onboard() -> World {
  let rk = random_signing_key();
  let owner = Device::new(ACCOUNT, 1);
  let shop = Device::new("shop", 1);
  let guardian_keys: Vec<SigningKey> = (0..GUARDIANS).map(|_| random_signing_key()).collect();
  let raw = shares::split(&rk, GUARDIANS);
  let sealed_shares: Vec<SealedShare> = raw
    .iter()
    .zip(&guardian_keys)
    .enumerate()
    .map(|(i, (share, gk))| {
      let id = format!("g{i}");
      seal(
        share,
        ACCOUNT,
        &id,
        1,
        gk.verifying_key(),
        &owner.payment_key,
      )
      .unwrap()
    })
    .collect();
  let opened = sealed_shares
    .iter()
    .zip(&guardian_keys)
    .enumerate()
    .map(|(i, (sealed, gk))| {
      let id = format!("g{i}");
      open(sealed, gk, &owner.payment_public(), ACCOUNT, &id, 1).unwrap()
    })
    .collect();
  let mut ledger = Ledger::new(random_signing_key());
  ledger.register(
    ACCOUNT,
    *rk.verifying_key(),
    owner.payment_public(),
    BALANCE,
  );
  ledger.register(
    "shop",
    *random_signing_key().verifying_key(),
    shop.payment_public(),
    0,
  );
  World {
    rk_public: *rk.verifying_key(),
    owner,
    shop,
    guardian_keys,
    sealed_shares,
    opened,
    ledger,
  }
}

fn pick(world: &World, idx: &[usize]) -> Vec<Vec<u8>> {
  idx.iter().map(|i| world.opened[*i].clone()).collect()
}

fn random_subset(size: usize) -> Vec<usize> {
  let mut pool: Vec<usize> = (0..GUARDIANS).collect();
  let mut out = Vec::new();
  while out.len() < size {
    let mut b = [0u8; 1];
    getrandom::fill(&mut b).unwrap();
    out.push(pool.remove(b[0] as usize % pool.len()));
  }
  out
}

fn recover(world: &World, idx: &[usize]) -> Result<(Device, RecoveryCert), RecoveryError> {
  Device::recover(
    ACCOUNT,
    1,
    &pick(world, idx),
    &world.rk_public,
    b"attest".to_vec(),
  )
}

fn recover_from_guardians(
  world: &World,
  idx: &[usize],
) -> Result<(Device, RecoveryCert), RecoveryError> {
  let opened: Vec<Vec<u8>> = idx
    .iter()
    .map(|i| {
      let id = format!("g{i}");
      open(
        &world.sealed_shares[*i],
        &world.guardian_keys[*i],
        &world.owner.payment_public(),
        ACCOUNT,
        &id,
        1,
      )
    })
    .collect::<Result<_, _>>()?;
  Device::recover(ACCOUNT, 1, &opened, &world.rk_public, b"attest".to_vec())
}

/// Fresh recovery cert (new device key each time) submitted at `now`; returns cert + request_no.
fn submit(world: &mut World, now: u64) -> (RecoveryCert, u64) {
  let cert = recover(world, &[0, 1, 2]).unwrap().1;
  world.ledger.submit_recovery(cert.clone(), now).unwrap();
  (cert, world.ledger.pending_request_no(ACCOUNT).unwrap())
}

fn cancel_for(world: &World, cert: &RecoveryCert, request_no: u64) -> RecoveryCancel {
  RecoveryCancel::sign(&world.owner.payment_key, ACCOUNT, 1, cert, request_no)
}

fn cheque(id: u64, epoch: u64, amount: u64, expiry: u64, key: &SigningKey) -> Cheque {
  Cheque {
    id,
    payer_account: ACCOUNT.into(),
    payee_account: "shop".into(),
    epoch,
    amount,
    expiry,
    signature: Vec::new(),
  }
  .sign(key)
}

fn shop_pays_account(world: &World, id: u64) -> Cheque {
  Cheque {
    id,
    payer_account: "shop".into(),
    payee_account: ACCOUNT.into(),
    epoch: 1,
    amount: 5,
    expiry: FAR,
    signature: Vec::new(),
  }
  .sign(&world.shop.payment_key)
}

// ---------- PV-REC-01 ----------

#[test]
fn pv_rec_01_three_of_five_reconstructs_20_of_20_offline() {
  let world = onboard();
  let server = world.ledger.server_public();
  let snapshot = world.ledger.snapshot();
  for attempt in 0..20 {
    let (mut device, cert) = recover_from_guardians(&world, &random_subset(3)).unwrap();
    assert_eq!((cert.old_epoch, cert.new_epoch), (1, 2));
    assert_eq!(cert.account_id, ACCOUNT);
    verify(&world.rk_public, &cert.signed_bytes(), &cert.signature).unwrap();
    // test payment to the new device, verified against the signed snapshot
    device
      .receive(shop_pays_account(&world, attempt), &snapshot, &server, 0)
      .unwrap();
    // limitation: the old balance cannot be spent offline (100%)
    assert_eq!(device.spend_offline(1), Err(RecoveryError::NoAllowance));
  }
}

#[test]
fn every_subset_of_size_3_4_5_reconstructs() {
  let world = onboard();
  for mask in 0u8..32 {
    let idx: Vec<usize> = (0..5).filter(|i| mask >> i & 1 == 1).collect();
    let result = recover(&world, &idx);
    if idx.len() >= 3 {
      assert!(result.is_ok(), "subset {idx:?}");
    } else {
      assert!(result.is_err(), "subset {idx:?}");
    }
  }
}

// ---------- PV-REC-02 ----------

#[test]
fn pv_rec_02_two_shares_never_reconstruct_1000_random_secrets() {
  for _ in 0..1_000 {
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).unwrap();
    let all: Vec<_> = Sharks(3).dealer(&secret).take(5).collect();
    for a in 0..5 {
      for b in a + 1..5 {
        // attacker forces a combination with only 2 shares, bypassing the threshold check
        let forced = Sharks(2)
          .recover(&[all[a].clone(), all[b].clone()])
          .unwrap();
        assert_ne!(forced, secret);
        assert!(
          Sharks(3)
            .recover(&[all[a].clone(), all[b].clone()])
            .is_err()
        );
      }
    }
    assert_eq!(Sharks(3).recover(&all[1..4]).unwrap(), secret);
  }
}

#[test]
fn two_shares_rejected_by_combine() {
  let world = onboard();
  assert_eq!(
    shares::combine(&pick(&world, &[0, 1]), &world.rk_public).err(),
    Some(RecoveryError::NotEnoughShares)
  );
}

#[test]
fn wrong_subsets_fail_to_reconstruct() {
  let world = onboard();
  let other = onboard();
  let mut mixed = pick(&world, &[0, 1]);
  mixed.push(other.opened[2].clone());
  assert_eq!(
    shares::combine(&mixed, &world.rk_public).err(),
    Some(RecoveryError::WrongSecret)
  );
  let mut tampered = pick(&world, &[0, 1, 2]);
  tampered[1][5] ^= 1;
  assert_eq!(
    shares::combine(&tampered, &world.rk_public).err(),
    Some(RecoveryError::WrongSecret)
  );
  assert_eq!(
    shares::combine(&pick(&world, &[0, 0, 1]), &world.rk_public).err(),
    Some(RecoveryError::DuplicateShare)
  );
  assert_eq!(
    shares::combine(&[vec![1], vec![2, 3], vec![3, 4]], &world.rk_public).err(),
    Some(RecoveryError::BadShare)
  );
}

// ---------- share integrity / authentication (PRD 9.2) ----------

#[test]
fn sealed_share_is_authenticated_and_bound_to_guardian_account_epoch() {
  let owner = random_signing_key();
  let guardian = random_signing_key();
  let pubkey = owner.verifying_key();
  let share: Vec<u8> = (0..32u8).map(|i| 0xA5 ^ i.wrapping_mul(7)).collect();
  let sealed = seal(&share, ACCOUNT, "g0", 1, guardian.verifying_key(), &owner).unwrap();
  let ok = |s: &social_recovery::sealing::SealedShare| open(s, &guardian, pubkey, ACCOUNT, "g0", 1);
  assert_eq!(ok(&sealed).unwrap(), share);

  // ciphertext = share + 16-byte tag, no 8-byte window of the plaintext appears in it
  assert_eq!(sealed.ciphertext.len(), share.len() + 16);
  for window in share.windows(8) {
    assert!(!sealed.ciphertext.windows(8).any(|w| w == window));
  }
  // same share sealed twice: fresh ephemeral key and nonce
  let again = seal(&share, ACCOUNT, "g0", 1, guardian.verifying_key(), &owner).unwrap();
  assert_ne!(sealed.ciphertext, again.ciphertext);
  assert_ne!(sealed.ephemeral_public, again.ephemeral_public);

  // wrong guardian cannot decrypt (signature fine, AEAD fails)
  let stranger = random_signing_key();
  assert_eq!(
    open(&sealed, &stranger, pubkey, ACCOUNT, "g0", 1),
    Err(RecoveryError::Decrypt)
  );
  // forged owner
  assert_eq!(
    open(
      &sealed,
      &guardian,
      random_signing_key().verifying_key(),
      ACCOUNT,
      "g0",
      1
    ),
    Err(RecoveryError::BadSignature)
  );
  // ciphertext / nonce / owner signature flips
  let mut bad = seal(&share, ACCOUNT, "g0", 1, guardian.verifying_key(), &owner).unwrap();
  bad.ciphertext[0] ^= 1;
  assert_eq!(ok(&bad), Err(RecoveryError::BadSignature));
  let mut bad = seal(&share, ACCOUNT, "g0", 1, guardian.verifying_key(), &owner).unwrap();
  bad.nonce[0] ^= 1;
  assert_eq!(ok(&bad), Err(RecoveryError::BadSignature));
  // relabel (guardian / account / epoch) without the owner key: signature fails
  for relabel in 0..3 {
    let mut moved = seal(&share, ACCOUNT, "g0", 1, guardian.verifying_key(), &owner).unwrap();
    match relabel {
      0 => moved.guardian_id = "g1".into(),
      1 => moved.account_id = "acct-2".into(),
      _ => moved.epoch = 2,
    }
    assert_eq!(ok(&moved), Err(RecoveryError::BadSignature));
  }
  // relabel AND re-sign with a valid owner signature: expected-context check rejects it, and
  // when the guardian expects the new label the AEAD AAD still rejects it
  let mut resigned = seal(&share, ACCOUNT, "g0", 1, guardian.verifying_key(), &owner).unwrap();
  resigned.guardian_id = "g1".into();
  resigned.owner_signature = sign(&owner, &resigned.signed_bytes());
  assert_eq!(ok(&resigned), Err(RecoveryError::WrongContext));
  assert_eq!(
    open(&resigned, &guardian, pubkey, ACCOUNT, "g1", 1),
    Err(RecoveryError::Decrypt)
  );
}

#[test]
fn old_epoch_share_replay_is_rejected_by_guardian() {
  let owner = random_signing_key();
  let guardian = random_signing_key();
  let old = seal(
    &[7u8; 33],
    ACCOUNT,
    "g0",
    1,
    guardian.verifying_key(),
    &owner,
  )
  .unwrap();
  // after a rotation the guardian expects epoch 2 shares only
  assert_eq!(
    open(&old, &guardian, owner.verifying_key(), ACCOUNT, "g0", 2),
    Err(RecoveryError::WrongContext)
  );
  // a signature over the sealed bytes is not a valid signature over any other domain
  let cancel_like = RecoveryCancel::signed_bytes(
    ACCOUNT,
    1,
    &RecoveryCert::sign(&random_signing_key(), ACCOUNT, 1, vec![2; 33], vec![]),
    1,
  );
  assert_ne!(cancel_like, old.signed_bytes());
}

// ---------- PV-REC-03 / replay / rotation ----------

#[test]
fn recovery_rotates_epoch_and_revokes_old_key() {
  let mut world = onboard();
  let old_key = world.owner.payment_key.clone();
  let server = world.ledger.server_public();
  world
    .ledger
    .reserve(ACCOUNT, 1, 1, 100, 100 * HOUR, 0)
    .unwrap();
  world
    .ledger
    .reserve(ACCOUNT, 1, 2, 200, 120 * HOUR, 0)
    .unwrap();
  let (new_device, cert) = recover(&world, &[0, 2, 4]).unwrap();
  let now = HOUR;
  world.ledger.submit_recovery(cert, now).unwrap();
  // frozen: nobody gets a book during T_rec
  assert_eq!(
    world.ledger.reserve(ACCOUNT, 1, 3, 1, 130 * HOUR, now),
    Err(RecoveryError::Frozen)
  );
  assert_eq!(
    world.ledger.reserve(ACCOUNT, 2, 3, 1, 130 * HOUR, now),
    Err(RecoveryError::Frozen)
  );
  assert_eq!(
    world.ledger.snapshot().pending_recovery_accounts,
    vec![ACCOUNT.to_owned()]
  );
  assert_eq!(
    world.ledger.activate(ACCOUNT, now + T_REC - 1),
    Err(RecoveryError::TimeLockActive)
  );
  assert!(world.ledger.pending_request_no(ACCOUNT).is_some()); // failed activate keeps pending
  let active = now + T_REC;
  world.ledger.activate(ACCOUNT, active).unwrap();
  assert_eq!(world.ledger.epoch(ACCOUNT), Some(2));
  assert_eq!(
    world.ledger.reserve(ACCOUNT, 1, 4, 1, 190 * HOUR, active),
    Err(RecoveryError::WrongEpoch)
  );

  // receiver (shop) with the updated, signed snapshot: old key refused, new key accepted
  let snapshot = world.ledger.snapshot();
  let mut shop = Device::new("shop", 1);
  let late = cheque(50, 1, 10, FAR, &old_key);
  assert_eq!(
    shop.receive(late, &snapshot, &server, active),
    Err(RecoveryError::Revoked)
  );
  let fresh = cheque(51, 2, 10, FAR, &new_device.payment_key);
  assert_eq!(shop.receive(fresh, &snapshot, &server, active), Ok(()));
  // an attacker who kept the old key but signs as epoch 2 is refused by the signature check
  let spoof = cheque(52, 2, 10, FAR, &old_key);
  assert_eq!(
    shop.receive(spoof, &snapshot, &server, active),
    Err(RecoveryError::BadSignature)
  );

  // old cheques signed before revocation and deposited up to E are paid, once
  let one = cheque(1, 1, 100, 100 * HOUR, &old_key);
  assert_eq!(world.ledger.deposit(&one, active + HOUR), Ok(()));
  assert_eq!(
    world.ledger.deposit(&one, active + HOUR),
    Err(RecoveryError::AlreadyDeposited)
  );
  // unreserved cheque from the old device after revocation: refused
  assert_eq!(
    world
      .ledger
      .deposit(&cheque(77, 1, 10, FAR, &old_key), active),
    Err(RecoveryError::UnknownCheque)
  );
  // forged signature on a reserved id: refused
  let forged = cheque(2, 1, 200, 120 * HOUR, &random_signing_key());
  assert_eq!(
    world.ledger.deposit(&forged, active + HOUR),
    Err(RecoveryError::BadSignature)
  );
  // past E: refused
  let two = cheque(2, 1, 200, 120 * HOUR, &old_key);
  assert_eq!(
    world.ledger.deposit(&two, 120 * HOUR + 1),
    Err(RecoveryError::Expired)
  );
}

#[test]
fn pv_rec_03_updated_snapshot_rejects_100_old_payments_but_deposits_them() {
  let mut world = onboard();
  let old_key = world.owner.payment_key.clone();
  let server = world.ledger.server_public();
  let mut old_cheques = Vec::new();
  for id in 0..100 {
    world.ledger.reserve(ACCOUNT, 1, id, 1, FAR, 0).unwrap();
    old_cheques.push(cheque(id, 1, 1, FAR, &old_key));
  }

  let (_, cert) = recover(&world, &[0, 2, 4]).unwrap();
  world.ledger.submit_recovery(cert, HOUR).unwrap();
  let active = HOUR + T_REC;
  world.ledger.activate(ACCOUNT, active).unwrap();
  let snapshot = world.ledger.snapshot();
  assert_eq!(snapshot.accounts.get(ACCOUNT).unwrap().0, 2);
  let mut shop = Device::new("shop", 1);

  for old_cheque in &old_cheques {
    assert_eq!(
      shop.receive(old_cheque.clone(), &snapshot, &server, active),
      Err(RecoveryError::Revoked)
    );
    assert_eq!(world.ledger.deposit(old_cheque, active + HOUR), Ok(()));
  }
  assert!(shop.received.is_empty());
}

#[test]
fn snapshot_must_be_authentic_and_cheques_valid() {
  let mut world = onboard();
  let server = world.ledger.server_public();
  let key = world.owner.payment_key.clone();
  let (_, cert) = recover(&world, &[0, 1, 2]).unwrap();
  world.ledger.submit_recovery(cert, 0).unwrap();
  world.ledger.activate(ACCOUNT, T_REC).unwrap();
  let snapshot = world.ledger.snapshot();
  let mut shop = Device::new("shop", 1);
  let old = cheque(1, 1, 10, FAR, &key);

  // tampered snapshot (epoch rolled back to 1 so the revoked cheque would pass): signature fails
  let mut rolled_back = world.ledger.snapshot();
  let entry = rolled_back.accounts.get_mut(ACCOUNT).unwrap();
  entry.0 = 1;
  entry.1 = world
    .owner
    .payment_public()
    .to_sec1_point(true)
    .as_bytes()
    .to_vec();
  assert_eq!(
    shop.receive(old.clone(), &rolled_back, &server, T_REC),
    Err(RecoveryError::BadSignature)
  );
  // snapshot signed by a key that is not the server's
  let forged = rolled_back.sign(&random_signing_key());
  assert_eq!(
    shop.receive(old.clone(), &forged, &server, T_REC),
    Err(RecoveryError::BadSignature)
  );
  // honest snapshot: revoked
  assert_eq!(
    shop.receive(old, &snapshot, &server, T_REC),
    Err(RecoveryError::Revoked)
  );
  // unknown payer, wrong payee, expired cheque
  let mut ghost = shop_pays_account(&world, 5);
  ghost.payer_account = "ghost".into();
  let mut recipient = Device::new(ACCOUNT, 2);
  assert_eq!(
    recipient.receive(ghost, &snapshot, &server, 0),
    Err(RecoveryError::UnknownAccount)
  );
  let for_someone_else = shop_pays_account(&world, 6);
  assert_eq!(
    shop.receive(for_someone_else, &snapshot, &server, 0),
    Err(RecoveryError::NotPayee)
  );
  let mut expired = shop_pays_account(&world, 7);
  expired.expiry = 10;
  let expired = expired.sign(&world.shop.payment_key);
  assert_eq!(
    recipient.receive(expired, &snapshot, &server, 11),
    Err(RecoveryError::Expired)
  );
}

#[test]
fn replayed_or_skipped_epoch_cert_is_refused() {
  let mut world = onboard();
  let (cert, _) = {
    let cert = recover(&world, &[0, 1, 2]).unwrap().1;
    (cert, ())
  };
  world.ledger.submit_recovery(cert.clone(), 0).unwrap();
  // same signed cert while pending
  assert_eq!(
    world.ledger.submit_recovery(cert.clone(), 1),
    Err(RecoveryError::AlreadyPending)
  );
  world.ledger.activate(ACCOUNT, T_REC).unwrap();
  // same signed cert again after the epoch moved
  assert_eq!(
    world.ledger.submit_recovery(cert, 40 * DAY),
    Err(RecoveryError::StaleEpoch)
  );
  // cert signed by someone who is not RK
  let forged = RecoveryCert::sign(&random_signing_key(), ACCOUNT, 2, vec![2; 33], vec![]);
  assert_eq!(
    world.ledger.submit_recovery(forged, 40 * DAY),
    Err(RecoveryError::BadSignature)
  );
  // RK-signed cert skipping an epoch
  let rk = shares::combine(&pick(&world, &[1, 2, 3]), &world.rk_public).unwrap();
  let mut skip = RecoveryCert::sign(&rk, ACCOUNT, 2, vec![2; 33], vec![]);
  skip.new_epoch = 4;
  skip.signature = sign(&rk, &skip.signed_bytes());
  assert_eq!(
    world.ledger.submit_recovery(skip, 40 * DAY),
    Err(RecoveryError::StaleEpoch)
  );
}

#[test]
fn bad_new_device_pubkey_is_rejected_at_submit_without_side_effects() {
  let mut world = onboard();
  let rk = shares::combine(&pick(&world, &[0, 1, 2]), &world.rk_public).unwrap();
  for garbage in [
    vec![],
    vec![0u8; 33],
    [&[3u8][..], &[0xFFu8; 32][..]].concat(),
    vec![4u8; 65],
  ] {
    let cert = RecoveryCert::sign(&rk, ACCOUNT, 1, garbage, vec![]);
    assert_eq!(
      world.ledger.submit_recovery(cert, 0),
      Err(RecoveryError::BadKey)
    );
    assert_eq!(world.ledger.pending_request_no(ACCOUNT), None);
  }
  // none of those burned the rate limit or froze the account: 3 real requests still fit
  world.ledger.reserve(ACCOUNT, 1, 1, 1, HOUR, 0).unwrap();
  let mut t = 0;
  for _ in 0..3 {
    let (cert, n) = submit(&mut world, t);
    world
      .ledger
      .cancel(&cancel_for(&world, &cert, n), t + HOUR)
      .unwrap();
    t += CANCEL_BLOCK + DAY;
  }
}

// ---------- PV-REC-04 ----------

#[test]
fn pv_rec_04_quarantine_releases_after_last_expiry_with_zero_balance_diff() {
  let mut world = onboard();
  let old_key = world.owner.payment_key.clone();
  world
    .ledger
    .reserve(ACCOUNT, 1, 1, 100, 100 * HOUR, 0)
    .unwrap();
  world
    .ledger
    .reserve(ACCOUNT, 1, 2, 250, 120 * HOUR, 0)
    .unwrap();
  let last_expiry = 120 * HOUR;
  let (_, cert) = recover(&world, &[0, 1, 2]).unwrap();
  world.ledger.submit_recovery(cert, 0).unwrap();
  world.ledger.activate(ACCOUNT, T_REC).unwrap();
  world
    .ledger
    .deposit(&cheque(1, 1, 100, 100 * HOUR, &old_key), T_REC + HOUR)
    .unwrap();
  assert_eq!(
    world.ledger.available(ACCOUNT, T_REC),
    Ok(BALANCE - 100 - 250)
  );
  assert_eq!(
    world.ledger.available(ACCOUNT, last_expiry - 1),
    Ok(BALANCE - 100 - 250)
  );
  // 250 released at the last expiry, never deposited: available + paid == original balance
  assert_eq!(
    world.ledger.available(ACCOUNT, last_expiry),
    Ok(BALANCE - 100)
  );
}

#[test]
fn quarantine_withholds_until_last_old_cheque_expires() {
  let mut world = onboard();
  world
    .ledger
    .reserve(ACCOUNT, 1, 1, 300, 100 * HOUR, 0)
    .unwrap();
  let (_, cert) = recover(&world, &[0, 1, 2]).unwrap();
  world.ledger.submit_recovery(cert, 0).unwrap();
  world.ledger.activate(ACCOUNT, T_REC).unwrap();
  assert_eq!(world.ledger.available(ACCOUNT, T_REC), Ok(BALANCE - 300));
  assert_eq!(
    world
      .ledger
      .reserve(ACCOUNT, 2, 10, BALANCE - 299, 200 * HOUR, T_REC),
    Err(RecoveryError::InsufficientFunds)
  );
  world
    .ledger
    .reserve(ACCOUNT, 2, 10, BALANCE - 300, 200 * HOUR, T_REC)
    .unwrap();
  assert_eq!(world.ledger.available(ACCOUNT, 100 * HOUR), Ok(300));
}

// ---------- duplicate cheque ids ----------

#[test]
fn reserve_rejects_reserved_and_deposited_cheque_ids() {
  let mut world = onboard();
  let key = world.owner.payment_key.clone();
  world
    .ledger
    .reserve(ACCOUNT, 1, 1, 100, 10 * HOUR, 0)
    .unwrap();
  assert_eq!(
    world.ledger.reserve(ACCOUNT, 1, 1, 50, 20 * HOUR, 0),
    Err(RecoveryError::DuplicateCheque)
  );
  world
    .ledger
    .deposit(&cheque(1, 1, 100, 10 * HOUR, &key), HOUR)
    .unwrap();
  assert_eq!(
    world.ledger.reserve(ACCOUNT, 1, 1, 100, 20 * HOUR, HOUR),
    Err(RecoveryError::DuplicateCheque)
  );
  // an expired but never deposited id stays reserved: no second cheque on the same id
  world
    .ledger
    .reserve(ACCOUNT, 1, 2, 10, 2 * HOUR, 0)
    .unwrap();
  assert_eq!(
    world.ledger.reserve(ACCOUNT, 1, 2, 10, 50 * HOUR, 3 * HOUR),
    Err(RecoveryError::DuplicateCheque)
  );
  // already-expired cheque cannot be issued
  assert_eq!(
    world.ledger.reserve(ACCOUNT, 1, 3, 10, 3 * HOUR, 3 * HOUR),
    Err(RecoveryError::Expired)
  );
  assert_eq!(world.ledger.available(ACCOUNT, 3 * HOUR), Ok(BALANCE - 100));
}

// ---------- PV-REC-05 (time-lock, cancel, rate limit) ----------

#[test]
fn cancel_rate_limit_and_block() {
  let mut world = onboard();
  let (cert, n) = submit(&mut world, 0);
  assert_eq!(
    world
      .ledger
      .submit_recovery(recover(&world, &[0, 1, 2]).unwrap().1, 1),
    Err(RecoveryError::AlreadyPending)
  );
  // invalid cancels: wrong key, wrong counter, wrong cert
  let wrong_key = RecoveryCancel::sign(&random_signing_key(), ACCOUNT, 1, &cert, n);
  assert_eq!(
    world.ledger.cancel(&wrong_key, HOUR),
    Err(RecoveryError::BadSignature)
  );
  let wrong_n = cancel_for(&world, &cert, n + 1);
  assert_eq!(
    world.ledger.cancel(&wrong_n, HOUR),
    Err(RecoveryError::BadSignature)
  );
  let other_cert = recover(&world, &[0, 1, 2]).unwrap().1;
  let wrong_cert = cancel_for(&world, &other_cert, n);
  assert_eq!(
    world.ledger.cancel(&wrong_cert, HOUR),
    Err(RecoveryError::BadSignature)
  );
  // valid cancel too late (>= T_rec) refused, in time accepted
  let valid = cancel_for(&world, &cert, n);
  assert_eq!(
    world.ledger.cancel(&valid, T_REC),
    Err(RecoveryError::TimeLockActive)
  );
  world.ledger.cancel(&valid, T_REC - 1).unwrap();
  assert_eq!(world.ledger.epoch(ACCOUNT), Some(1));
  // blocked for 7 days after a valid cancel
  let again = recover(&world, &[1, 2, 3]).unwrap().1;
  assert_eq!(
    world
      .ledger
      .submit_recovery(again, T_REC - 1 + CANCEL_BLOCK - 1),
    Err(RecoveryError::Blocked)
  );
  // after the block: accepted, activates; a 2nd effective one within 30 days is refused
  let t = T_REC + CANCEL_BLOCK;
  world
    .ledger
    .submit_recovery(recover(&world, &[1, 2, 3]).unwrap().1, t)
    .unwrap();
  world.ledger.activate(ACCOUNT, t + T_REC).unwrap();
  let rk = shares::combine(&pick(&world, &[0, 1, 2]), &world.rk_public).unwrap();
  let next = RecoveryCert::sign(
    &rk,
    ACCOUNT,
    2,
    world
      .shop
      .payment_public()
      .to_sec1_point(true)
      .as_bytes()
      .to_vec(),
    vec![],
  );
  assert_eq!(
    world.ledger.submit_recovery(next, t + T_REC + DAY),
    Err(RecoveryError::RateLimited)
  );
}

#[test]
fn pv_rec_05_timelock_cancel_and_notice_hold_for_50_requests() {
  let mut world = onboard();
  let server = world.ledger.server_public();
  let mut now = 0;

  for case in 0..50 {
    let (cert, request_no) = submit(&mut world, now);
    let pending_snapshot = world.ledger.snapshot();
    assert_eq!(
      pending_snapshot.pending_recovery_accounts,
      vec![ACCOUNT.to_owned()],
      "old device notice missing in case {case}"
    );
    verify(
      &server,
      &pending_snapshot.signed_bytes(),
      &pending_snapshot.signature,
    )
    .unwrap();

    assert_eq!(
      world.ledger.activate(ACCOUNT, now + T_REC - 1),
      Err(RecoveryError::TimeLockActive)
    );
    for epoch in [1, 2] {
      assert_eq!(
        world
          .ledger
          .reserve(ACCOUNT, epoch, case, 1, now + T_REC + HOUR, now),
        Err(RecoveryError::Frozen),
        "epoch {epoch} received a talon before T_rec in case {case}"
      );
    }

    let other_key_cancel =
      RecoveryCancel::sign(&world.shop.payment_key, ACCOUNT, 1, &cert, request_no);
    assert_eq!(
      world.ledger.cancel(&other_key_cancel, now + HOUR),
      Err(RecoveryError::BadSignature)
    );
    let other_account_cancel =
      RecoveryCancel::sign(&world.shop.payment_key, "shop", 1, &cert, request_no);
    assert_eq!(
      world.ledger.cancel(&other_account_cancel, now + HOUR),
      Err(RecoveryError::NoPending)
    );
    assert_eq!(world.ledger.pending_request_no(ACCOUNT), Some(request_no));

    let cancel_at = now + T_REC - 1;
    world
      .ledger
      .cancel(&cancel_for(&world, &cert, request_no), cancel_at)
      .unwrap();
    assert_eq!(world.ledger.pending_request_no(ACCOUNT), None);
    assert!(world.ledger.snapshot().pending_recovery_accounts.is_empty());
    now += RATE_WINDOW;
  }
}

#[test]
fn pv_rec_05_second_effective_recovery_is_refused_for_50_cycles() {
  let mut world = onboard();
  let mut submitted_at = 0;

  for epoch in 1..=50 {
    let (_, cert) = Device::recover(
      ACCOUNT,
      epoch,
      &pick(&world, &[0, 1, 2]),
      &world.rk_public,
      b"attest".to_vec(),
    )
    .unwrap();
    world.ledger.submit_recovery(cert, submitted_at).unwrap();
    let effective_at = submitted_at + T_REC;
    world.ledger.activate(ACCOUNT, effective_at).unwrap();
    assert_eq!(world.ledger.epoch(ACCOUNT), Some(epoch + 1));

    let (_, second_cert) = Device::recover(
      ACCOUNT,
      epoch + 1,
      &pick(&world, &[0, 1, 2]),
      &world.rk_public,
      b"attest".to_vec(),
    )
    .unwrap();
    assert_eq!(
      world.ledger.submit_recovery(second_cert, effective_at + 1),
      Err(RecoveryError::RateLimited)
    );

    submitted_at = effective_at + RATE_WINDOW;
  }
}

#[test]
fn cancel_for_one_pending_cannot_be_replayed_against_another() {
  let mut world = onboard();
  let (cert1, n1) = submit(&mut world, 0);
  let cancel1 = cancel_for(&world, &cert1, n1);
  world.ledger.cancel(&cancel1, HOUR).unwrap();
  let mut t = CANCEL_BLOCK + DAY;

  // a different cert, new pending
  let (cert2, n2) = submit(&mut world, t);
  assert_ne!(n1, n2);
  for _ in 0..5 {
    assert_eq!(
      world.ledger.cancel(&cancel1, t + HOUR),
      Err(RecoveryError::BadSignature)
    );
  }
  assert_eq!(world.ledger.pending_request_no(ACCOUNT), Some(n2)); // still pending
  world
    .ledger
    .cancel(&cancel_for(&world, &cert2, n2), t + HOUR)
    .unwrap();
  t += CANCEL_BLOCK + DAY;

  // the IDENTICAL cert as cert1 replayed by a guardian cabal: new request number, so cancel1
  // (valid for request n1) does not cancel it; only a fresh cancel does
  world.ledger.submit_recovery(cert1.clone(), t).unwrap();
  let n3 = world.ledger.pending_request_no(ACCOUNT).unwrap();
  assert!(n3 > n2);
  assert_eq!(
    world.ledger.cancel(&cancel1, t + HOUR),
    Err(RecoveryError::BadSignature)
  );
  assert_eq!(world.ledger.pending_request_no(ACCOUNT), Some(n3));
  world
    .ledger
    .cancel(&cancel_for(&world, &cert1, n3), t + HOUR)
    .unwrap();

  // replays burned nothing: exactly 3 requests were counted, so a 4th inside 30 days is limited
  t += CANCEL_BLOCK + DAY;
  assert_eq!(
    world
      .ledger
      .submit_recovery(recover(&world, &[0, 1, 2]).unwrap().1, t),
    Err(RecoveryError::RateLimited)
  );
}

#[test]
fn at_most_three_requests_per_30_days() {
  let mut world = onboard();
  let mut t = 0;
  for _ in 0..3 {
    let (cert, n) = submit(&mut world, t);
    world
      .ledger
      .cancel(&cancel_for(&world, &cert, n), t + HOUR)
      .unwrap();
    t += CANCEL_BLOCK + DAY;
  }
  let cert = recover(&world, &[0, 1, 2]).unwrap().1;
  assert_eq!(
    world.ledger.submit_recovery(cert.clone(), t),
    Err(RecoveryError::RateLimited)
  );
  // the window slides: after 30 days from the first request a new one is accepted
  assert_eq!(world.ledger.submit_recovery(cert, 31 * DAY), Ok(()));
}

// ---------- overflow safety ----------

#[test]
fn extreme_u64_values_do_not_panic() {
  let mut world = onboard();
  world.ledger.register(
    "big",
    *random_signing_key().verifying_key(),
    *random_signing_key().verifying_key(),
    u64::MAX,
  );
  world
    .ledger
    .reserve("big", 1, 1, u64::MAX, u64::MAX, 0)
    .unwrap();
  assert_eq!(
    world.ledger.reserve("big", 1, 2, 1, u64::MAX, 0),
    Err(RecoveryError::InsufficientFunds)
  );
  assert_eq!(world.ledger.available("big", 0), Ok(0));
  assert_eq!(world.ledger.available("big", u64::MAX), Ok(u64::MAX));

  // now + T_REC overflows: refused, nothing changes
  let cert = recover(&world, &[0, 1, 2]).unwrap().1;
  assert_eq!(
    world.ledger.submit_recovery(cert.clone(), u64::MAX),
    Err(RecoveryError::Overflow)
  );
  assert_eq!(world.ledger.pending_request_no(ACCOUNT), None);
  // the largest accepted `now`: time + window and block saturate instead of panicking
  let edge = u64::MAX - T_REC;
  world.ledger.submit_recovery(cert.clone(), edge).unwrap();
  let n = world.ledger.pending_request_no(ACCOUNT).unwrap();
  world
    .ledger
    .cancel(&cancel_for(&world, &cert, n), edge + 1)
    .unwrap();
  assert_eq!(
    world
      .ledger
      .submit_recovery(recover(&world, &[0, 1, 2]).unwrap().1, u64::MAX - 1),
    Err(RecoveryError::Blocked)
  );
}

// ---------- 9.4 limitation (documented, tested, not hidden) ----------

#[test]
fn long_outage_recovered_device_receives_but_cannot_spend_old_balance() {
  let mut world = onboard();
  let server = world.ledger.server_public();
  let (mut device, cert) = recover(&world, &[2, 3, 4]).unwrap();
  // server unreachable: ledger never touched, epoch still 1, receivers keep the old snapshot
  assert_eq!(world.ledger.epoch(ACCOUNT), Some(1));
  assert_eq!(device.spend_offline(1), Err(RecoveryError::NoAllowance));
  let snapshot = world.ledger.snapshot();
  device
    .receive(shop_pays_account(&world, 1), &snapshot, &server, 0)
    .unwrap();
  assert_eq!(device.spend_offline(5), Err(RecoveryError::NoAllowance)); // received != spendable
  // server returns: only now do the time-lock and the revocation start
  let back = 1_000 * DAY;
  world.ledger.submit_recovery(cert, back).unwrap();
  assert_eq!(world.ledger.epoch(ACCOUNT), Some(1));
  world.ledger.activate(ACCOUNT, back + T_REC).unwrap();
  device.grant_allowance(world.ledger.available(ACCOUNT, back + T_REC).unwrap());
  assert_eq!(device.spend_offline(BALANCE), Ok(()));
}
