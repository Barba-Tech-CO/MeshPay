use crate::crypto::spend_hashing::spend_hash;
use crate::error::ApiError;
use crate::ledger::units::to_u64;
use crate::wire::cheque_spend::ChequeSpend;
use crate::wire::cheque_stub::ChequeStub;
use crate::wire::conflict_kind::ConflictKind;
use crate::wire::equivocation_proof::EquivocationProof;
use prost::Message;
use sqlx::PgConnection;

/// Stores an `EquivocationProof` (PRD 5.1) and freezes the offender (no new talões).
/// A repeated identical conflict stores nothing new. Runs in the caller's transaction.
/// Returns whether the proof is new; the caller appends the audit entry AFTER its ledger
/// postings, because every transaction takes the supply row before the audit head (no deadlock).
pub async fn record_proof(
  connection: &mut PgConnection,
  kind: ConflictKind,
  offender: &[u8],
  epoch: i64,
  first: (&ChequeStub, &ChequeSpend),
  second: (&ChequeStub, &ChequeSpend),
) -> Result<(EquivocationProof, bool), ApiError> {
  let kind_name = match kind {
    ConflictKind::SameCheque => "SAME_CHEQUE",
    _ => "SAME_COUNTER",
  };
  let inserted = sqlx::query(
    "INSERT INTO equivocation_proofs (offender_account_id, epoch, conflict_kind, stub_1, spend_1,
       stub_2, spend_2, body_hash_1, body_hash_2) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
     ON CONFLICT DO NOTHING",
  )
  .bind(offender)
  .bind(epoch)
  .bind(kind_name)
  .bind(first.0.encode_to_vec())
  .bind(first.1.encode_to_vec())
  .bind(second.0.encode_to_vec())
  .bind(second.1.encode_to_vec())
  .bind(spend_hash(&first.1.body))
  .bind(spend_hash(&second.1.body))
  .execute(&mut *connection)
  .await?
  .rows_affected()
    == 1;
  sqlx::query("UPDATE accounts SET frozen = true WHERE account_id = $1")
    .bind(offender)
    .execute(&mut *connection)
    .await?;
  Ok((
    EquivocationProof {
      offender_account_id: offender.to_vec(),
      epoch: to_u64(epoch)?,
      conflict_kind: kind as i32,
      stub_1: Some(first.0.clone()),
      spend_1: Some(first.1.clone()),
      stub_2: Some(second.0.clone()),
      spend_2: Some(second.1.clone()),
    },
    inserted,
  ))
}
