-- PV-NET-04: a deposit now locks only the payer's account row (src/ledger/deposit.rs), so many
-- payers can pay one payee at once. Every ledger write then takes the supply singleton BEFORE any
-- balances row: one global lock order (accounts -> supply -> balances -> audit_head), so deposits
-- in opposite directions (A pays B while B pays A) cannot deadlock on each other's balance row.
CREATE OR REPLACE FUNCTION apply_ledger_entry() RETURNS trigger LANGUAGE plpgsql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
  UPDATE supply
    SET granted = granted + CASE WHEN NEW.kind = 'GRANT' THEN NEW.delta_available ELSE 0 END,
        circulating = circulating + NEW.delta_available + NEW.delta_reserved;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'supply row missing';
  END IF;
  UPDATE balances
    SET available = available + NEW.delta_available,
        reserved = reserved + NEW.delta_reserved
    WHERE account_id = NEW.account_id;
  RETURN NEW;
END $$;
