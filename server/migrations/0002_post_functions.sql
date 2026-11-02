-- The only way the runtime role moves money. Each function derives every amount and account
-- from an existing source row, so a caller can only choose WHICH row to post, never the value.
-- validate_ledger_entry re-checks each entry; ledger_entries_once stops repeats.
CREATE FUNCTION post_register(p_public_key bytea) RETURNS boolean LANGUAGE plpgsql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
  inserted bigint;
BEGIN
  INSERT INTO accounts (account_id, public_key, attestation_level)
  VALUES (substring(sha256(p_public_key) FROM 1 FOR 16), p_public_key, 'LAB_SOFTWARE')
  ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS inserted = ROW_COUNT;
  RETURN inserted = 1;
END $$;

CREATE FUNCTION post_reserve(p_chequebook_id bytea) RETURNS void LANGUAGE plpgsql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
  inserted bigint;
BEGIN
  INSERT INTO ledger_entries (account_id, kind, delta_available, delta_reserved, ref_id)
  SELECT b.account_id, 'RESERVE', -b.reserve_units, b.reserve_units, b.chequebook_id
  FROM chequebooks b WHERE b.chequebook_id = p_chequebook_id;
  GET DIAGNOSTICS inserted = ROW_COUNT;
  IF inserted <> 1 THEN
    RAISE EXCEPTION 'unknown chequebook';
  END IF;
END $$;

CREATE FUNCTION post_redeem(p_cheque_id bytea) RETURNS void LANGUAGE plpgsql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
  inserted bigint;
BEGIN
  INSERT INTO ledger_entries (account_id, kind, delta_available, delta_reserved, ref_id)
  SELECT r.payer_account_id, 'SETTLE', c.max_amount_units - r.amount_units,
         -c.max_amount_units, r.cheque_id
  FROM redemptions r JOIN cheques c USING (cheque_id) WHERE r.cheque_id = p_cheque_id;
  GET DIAGNOSTICS inserted = ROW_COUNT;
  IF inserted <> 1 THEN
    RAISE EXCEPTION 'unknown redemption';
  END IF;
  INSERT INTO ledger_entries (account_id, kind, delta_available, delta_reserved, ref_id)
  SELECT r.payee_account_id, 'CREDIT', r.amount_units, 0, r.cheque_id
  FROM redemptions r WHERE r.cheque_id = p_cheque_id;
END $$;

CREATE FUNCTION post_release(p_cheque_id bytea) RETURNS void LANGUAGE plpgsql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
  inserted bigint;
BEGIN
  INSERT INTO ledger_entries (account_id, kind, delta_available, delta_reserved, ref_id)
  SELECT c.account_id, 'RELEASE', l.released_units, -l.released_units, l.cheque_id
  FROM cheque_releases l JOIN cheques c USING (cheque_id) WHERE l.cheque_id = p_cheque_id;
  GET DIAGNOSTICS inserted = ROW_COUNT;
  IF inserted <> 1 THEN
    RAISE EXCEPTION 'unknown release';
  END IF;
END $$;
