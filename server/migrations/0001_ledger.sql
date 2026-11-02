-- MeshPay lab ledger. Typed columns only (no json/jsonb). Ledger tables are append-only.
-- Runs as the non-superuser schema owner `meshpay_owner` (see db/pool.rs). Money can only enter
-- through the account-created trigger (initial grant stored in `supply`); every other ledger
-- entry is validated against its source row (migration 0002).

CREATE TABLE accounts (
  account_id bytea PRIMARY KEY CHECK (length(account_id) = 16),
  public_key bytea NOT NULL UNIQUE CHECK (length(public_key) = 33),
  attestation_level text NOT NULL CHECK (attestation_level IN ('LAB_SOFTWARE')),
  epoch bigint NOT NULL DEFAULT 1 CHECK (epoch >= 1),
  frozen boolean NOT NULL DEFAULT false,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE balances (
  account_id bytea PRIMARY KEY REFERENCES accounts (account_id),
  available bigint NOT NULL DEFAULT 0 CHECK (available >= 0),
  reserved bigint NOT NULL DEFAULT 0 CHECK (reserved >= 0)
);

-- Runs inside the account INSERT (trigger depth 1); its ledger insert is depth 2, which is the
-- only depth at which validate_ledger_entry accepts a GRANT.
CREATE FUNCTION on_account_created() RETURNS trigger LANGUAGE plpgsql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
  grant_units bigint;
BEGIN
  INSERT INTO balances (account_id) VALUES (NEW.account_id);
  SELECT initial_grant INTO grant_units FROM supply;
  IF grant_units > 0 THEN
    INSERT INTO ledger_entries (account_id, kind, delta_available, delta_reserved, ref_id)
    VALUES (NEW.account_id, 'GRANT', grant_units, 0, NEW.account_id);
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER accounts_created AFTER INSERT ON accounts
  FOR EACH ROW EXECUTE FUNCTION on_account_created();

-- Supply invariant (PRD 14.2): balances + reserves <= granted <= cap, enforced by CHECKs.
CREATE TABLE supply (
  singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
  granted bigint NOT NULL DEFAULT 0 CHECK (granted >= 0),
  circulating bigint NOT NULL DEFAULT 0 CHECK (circulating >= 0),
  cap bigint NOT NULL CHECK (cap >= 0),
  initial_grant bigint NOT NULL CHECK (initial_grant >= 0),
  CONSTRAINT supply_circulating_within_granted CHECK (circulating <= granted),
  CONSTRAINT supply_granted_within_cap CHECK (granted <= cap)
);

CREATE TABLE ledger_entries (
  entry_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  account_id bytea NOT NULL REFERENCES accounts (account_id),
  kind text NOT NULL CHECK (kind IN ('GRANT', 'RESERVE', 'SETTLE', 'CREDIT', 'RELEASE')),
  delta_available bigint NOT NULL,
  delta_reserved bigint NOT NULL,
  ref_id bytea NOT NULL CHECK (length(ref_id) = 16),
  created_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT ledger_entry_shape CHECK (
    CASE kind
      WHEN 'GRANT' THEN delta_available > 0 AND delta_reserved = 0
      WHEN 'CREDIT' THEN delta_available > 0 AND delta_reserved = 0
      WHEN 'RESERVE' THEN delta_available < 0 AND delta_reserved = -delta_available
      WHEN 'RELEASE' THEN delta_available > 0 AND delta_reserved = -delta_available
      WHEN 'SETTLE' THEN delta_available >= 0 AND delta_reserved < 0
        AND delta_available + delta_reserved < 0
      ELSE false
    END
  )
);

-- One entry per kind and source row: a settlement, credit, release or reserve cannot repeat.
CREATE UNIQUE INDEX ledger_entries_once ON ledger_entries (kind, ref_id);

CREATE FUNCTION apply_ledger_entry() RETURNS trigger LANGUAGE plpgsql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
  UPDATE balances
    SET available = available + NEW.delta_available,
        reserved = reserved + NEW.delta_reserved
    WHERE account_id = NEW.account_id;
  UPDATE supply
    SET granted = granted + CASE WHEN NEW.kind = 'GRANT' THEN NEW.delta_available ELSE 0 END,
        circulating = circulating + NEW.delta_available + NEW.delta_reserved;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'supply row missing';
  END IF;
  RETURN NEW;
END $$;

CREATE FUNCTION validate_ledger_entry() RETURNS trigger LANGUAGE plpgsql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
  ok boolean;
BEGIN
  IF NEW.kind = 'GRANT' THEN
    ok := pg_trigger_depth() >= 2 AND NEW.ref_id = NEW.account_id
      AND NEW.delta_available = (SELECT initial_grant FROM supply);
  ELSIF NEW.kind = 'RESERVE' THEN
    ok := EXISTS (
      SELECT 1 FROM chequebooks b
      WHERE b.chequebook_id = NEW.ref_id AND b.account_id = NEW.account_id
        AND b.reserve_units = NEW.delta_reserved);
  ELSIF NEW.kind = 'SETTLE' THEN
    ok := EXISTS (
      SELECT 1 FROM redemptions r JOIN cheques c USING (cheque_id)
      WHERE r.cheque_id = NEW.ref_id AND r.payer_account_id = NEW.account_id
        AND NEW.delta_reserved = -c.max_amount_units
        AND NEW.delta_available = c.max_amount_units - r.amount_units)
      AND NOT EXISTS (SELECT 1 FROM cheque_releases l WHERE l.cheque_id = NEW.ref_id);
  ELSIF NEW.kind = 'CREDIT' THEN
    ok := EXISTS (
      SELECT 1 FROM redemptions r
      WHERE r.cheque_id = NEW.ref_id AND r.payee_account_id = NEW.account_id
        AND r.amount_units = NEW.delta_available);
  ELSIF NEW.kind = 'RELEASE' THEN
    ok := EXISTS (
      SELECT 1 FROM cheque_releases l JOIN cheques c USING (cheque_id)
      WHERE l.cheque_id = NEW.ref_id AND c.account_id = NEW.account_id
        AND l.released_units = NEW.delta_available)
      AND NOT EXISTS (SELECT 1 FROM redemptions r WHERE r.cheque_id = NEW.ref_id);
  END IF;
  IF NOT coalesce(ok, false) THEN
    RAISE EXCEPTION 'ledger entry does not match its source row';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER ledger_entries_validate BEFORE INSERT ON ledger_entries
  FOR EACH ROW EXECUTE FUNCTION validate_ledger_entry();

CREATE TRIGGER ledger_entries_apply AFTER INSERT ON ledger_entries
  FOR EACH ROW EXECUTE FUNCTION apply_ledger_entry();

CREATE TABLE chequebooks (
  chequebook_id bytea PRIMARY KEY CHECK (length(chequebook_id) = 16),
  request_id bytea NOT NULL CHECK (length(request_id) = 16),
  account_id bytea NOT NULL REFERENCES accounts (account_id),
  epoch bigint NOT NULL,
  cheque_count integer NOT NULL CHECK (cheque_count BETWEEN 1 AND 10),
  max_cheque_units bigint NOT NULL CHECK (max_cheque_units > 0),
  reserve_units bigint NOT NULL,
  expires_at_unix bigint NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (account_id, request_id),
  CHECK (reserve_units = max_cheque_units * cheque_count)
);

CREATE TABLE cheques (
  cheque_id bytea PRIMARY KEY CHECK (length(cheque_id) = 16),
  chequebook_id bytea NOT NULL REFERENCES chequebooks (chequebook_id),
  account_id bytea NOT NULL REFERENCES accounts (account_id),
  epoch bigint NOT NULL,
  signer_pubkey bytea NOT NULL CHECK (length(signer_pubkey) = 33),
  max_amount_units bigint NOT NULL CHECK (max_amount_units > 0),
  expires_at_unix bigint NOT NULL,
  stub bytea NOT NULL
);

CREATE TABLE cheque_releases (
  cheque_id bytea PRIMARY KEY REFERENCES cheques (cheque_id),
  released_units bigint NOT NULL CHECK (released_units > 0),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX cheques_account_expiry ON cheques (account_id, expires_at_unix);
CREATE INDEX cheques_chequebook ON cheques (chequebook_id);

CREATE TABLE redemptions (
  cheque_id bytea PRIMARY KEY REFERENCES cheques (cheque_id),
  payer_account_id bytea NOT NULL REFERENCES accounts (account_id),
  payee_account_id bytea NOT NULL REFERENCES accounts (account_id),
  epoch bigint NOT NULL,
  counter bigint NOT NULL CHECK (counter >= 0),
  amount_units bigint NOT NULL CHECK (amount_units > 0),
  tx_id bytea NOT NULL CHECK (length(tx_id) = 32),
  body_hash bytea NOT NULL CHECK (length(body_hash) = 32),
  spend bytea NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  CHECK (payer_account_id <> payee_account_id)
);

CREATE INDEX redemptions_payer_counter ON redemptions (payer_account_id, epoch, counter);

CREATE TABLE equivocation_proofs (
  proof_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  offender_account_id bytea NOT NULL REFERENCES accounts (account_id),
  epoch bigint NOT NULL,
  conflict_kind text NOT NULL CHECK (conflict_kind IN ('SAME_CHEQUE', 'SAME_COUNTER')),
  stub_1 bytea NOT NULL,
  spend_1 bytea NOT NULL,
  stub_2 bytea NOT NULL,
  spend_2 bytea NOT NULL,
  body_hash_1 bytea NOT NULL CHECK (length(body_hash_1) = 32),
  body_hash_2 bytea NOT NULL CHECK (length(body_hash_2) = 32),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (conflict_kind, body_hash_1, body_hash_2)
);

-- Evidence rows are written by the runtime role. These checks keep them consistent with each
-- other; they cannot verify P-256 signatures, so forged-but-consistent rows remain a known lab
-- gap (see migration 0003).
CREATE FUNCTION validate_cheque_row() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM chequebooks b
    WHERE b.chequebook_id = NEW.chequebook_id AND b.account_id = NEW.account_id
      AND b.epoch = NEW.epoch AND b.max_cheque_units = NEW.max_amount_units
      AND b.expires_at_unix = NEW.expires_at_unix
  ) OR (SELECT count(*) FROM cheques WHERE chequebook_id = NEW.chequebook_id)
     >= (SELECT cheque_count FROM chequebooks WHERE chequebook_id = NEW.chequebook_id) THEN
    RAISE EXCEPTION 'cheque does not match its chequebook';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER cheques_validate BEFORE INSERT ON cheques
  FOR EACH ROW EXECUTE FUNCTION validate_cheque_row();

CREATE FUNCTION validate_redemption_row() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM cheques c
    WHERE c.cheque_id = NEW.cheque_id AND c.account_id = NEW.payer_account_id
      AND c.epoch = NEW.epoch AND NEW.amount_units <= c.max_amount_units
  ) OR EXISTS (SELECT 1 FROM cheque_releases l WHERE l.cheque_id = NEW.cheque_id) THEN
    RAISE EXCEPTION 'redemption does not match its cheque';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER redemptions_validate BEFORE INSERT ON redemptions
  FOR EACH ROW EXECUTE FUNCTION validate_redemption_row();

CREATE FUNCTION validate_release_row() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM cheques c
    WHERE c.cheque_id = NEW.cheque_id AND c.max_amount_units = NEW.released_units
      AND c.expires_at_unix < extract(epoch FROM now())::bigint
  ) OR EXISTS (SELECT 1 FROM redemptions r WHERE r.cheque_id = NEW.cheque_id) THEN
    RAISE EXCEPTION 'release does not match an expired, unredeemed cheque';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER cheque_releases_validate BEFORE INSERT ON cheque_releases
  FOR EACH ROW EXECUTE FUNCTION validate_release_row();

-- Hash-chained audit log (PRD 7.4). The chain head is advanced under a row lock.
CREATE TABLE audit_head (
  singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
  hash bytea NOT NULL CHECK (length(hash) = 32)
);

INSERT INTO audit_head (hash) VALUES (decode(repeat('00', 32), 'hex'));

CREATE TABLE audit_log (
  seq bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  event text NOT NULL CHECK (event IN (
    'ACCOUNT_REGISTERED', 'CHEQUEBOOK_ISSUED', 'CHEQUE_REDEEMED', 'CHEQUE_RELEASED',
    'EQUIVOCATION_PROVED'
  )),
  ref_id bytea NOT NULL,
  prev_hash bytea NOT NULL UNIQUE CHECK (length(prev_hash) = 32),
  hash bytea NOT NULL UNIQUE CHECK (length(hash) = 32),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE FUNCTION chain_audit_entry() RETURNS trigger LANGUAGE plpgsql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
  head bytea;
BEGIN
  SELECT hash INTO head FROM audit_head FOR UPDATE;
  NEW.prev_hash := head;
  NEW.hash := sha256(head || convert_to(NEW.event, 'UTF8') || NEW.ref_id);
  UPDATE audit_head SET hash = NEW.hash;
  RETURN NEW;
END $$;

CREATE TRIGGER audit_log_chain BEFORE INSERT ON audit_log
  FOR EACH ROW EXECUTE FUNCTION chain_audit_entry();

-- True when every hash recomputes and every link points at an existing hash or the genesis.
CREATE FUNCTION audit_chain_valid() RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT NOT EXISTS (
    SELECT 1 FROM audit_log a
    WHERE a.hash <> sha256(a.prev_hash || convert_to(a.event, 'UTF8') || a.ref_id)
  ) AND NOT EXISTS (
    SELECT 1 FROM audit_log a
    WHERE a.prev_hash <> decode(repeat('00', 32), 'hex')
      AND NOT EXISTS (SELECT 1 FROM audit_log b WHERE b.hash = a.prev_hash)
  )
$$;

-- Append-only: no UPDATE, DELETE or TRUNCATE on any ledger/evidence table.
CREATE FUNCTION forbid_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION '% is append-only', TG_TABLE_NAME;
END $$;

DO $$
DECLARE
  tbl text;
BEGIN
  FOREACH tbl IN ARRAY ARRAY[
    'ledger_entries', 'audit_log', 'chequebooks', 'cheques', 'cheque_releases',
    'redemptions', 'equivocation_proofs'
  ] LOOP
    EXECUTE format(
      'CREATE TRIGGER %I BEFORE UPDATE OR DELETE OR TRUNCATE ON %I
       FOR EACH STATEMENT EXECUTE FUNCTION forbid_mutation()',
      tbl || '_no_mutation', tbl);
  END LOOP;
END $$;

-- The supply cap is fixed at first start, for every role including the owner.
CREATE FUNCTION forbid_supply_cap_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.cap <> OLD.cap OR NEW.initial_grant <> OLD.initial_grant THEN
    RAISE EXCEPTION 'supply.cap and supply.initial_grant are immutable';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER supply_cap_immutable BEFORE UPDATE ON supply
  FOR EACH ROW EXECUTE FUNCTION forbid_supply_cap_change();

-- An account row may only change by freezing; it can never be unfrozen.
CREATE FUNCTION accounts_freeze_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.account_id <> OLD.account_id OR NEW.public_key <> OLD.public_key
     OR NEW.attestation_level <> OLD.attestation_level OR NEW.epoch <> OLD.epoch
     OR NEW.created_at <> OLD.created_at OR (OLD.frozen AND NOT NEW.frozen) THEN
    RAISE EXCEPTION 'accounts may only be frozen';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER accounts_freeze_only BEFORE UPDATE ON accounts
  FOR EACH ROW EXECUTE FUNCTION accounts_freeze_only();

-- The supply row is created here, in the same transaction as the schema, from session settings
-- `meshpay.supply_cap` and `meshpay.initial_grant` set by `meshpay_server migrate`. A missing
-- setting makes current_setting() raise, so the migration (and the migrate service) fails loudly.
INSERT INTO supply (cap, initial_grant)
VALUES (current_setting('meshpay.supply_cap')::bigint,
        current_setting('meshpay.initial_grant')::bigint);
