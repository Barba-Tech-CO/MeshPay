-- PV-SEC-05 (PRD 7.4, 14.2): ServerKeyCert registry, revoked server keys and the issuance kill
-- switch, plus a stricter audit chain verifier. All three tables are append-only: a compromised
-- runtime role can add a revocation or a freeze (denial of service only) but never remove one.

-- Root-signed certificates of the online keys, as received. The server re-verifies the root
-- signature, role and validity on every use, so a forged row is useless without the root key.
-- Keyed by cert hash: a renewed cert adds a row, and one key may be certified for several roles.
CREATE TABLE server_key_certs (
  key_id bytea NOT NULL CHECK (length(key_id) = 16),
  role smallint NOT NULL,
  cert_hash bytea NOT NULL CHECK (length(cert_hash) = 32),
  cert bytea NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (key_id, role, cert_hash)
);

-- `revoked_server_key_ids` from verified snapshots (signed by a SNAPSHOT key or the root).
CREATE TABLE revoked_server_keys (
  key_id bytea PRIMARY KEY CHECK (length(key_id) = 16),
  snapshot_hash bytea NOT NULL CHECK (length(snapshot_hash) = 32),
  created_at timestamptz NOT NULL DEFAULT now()
);

-- `issuance_frozen = true` snapshots. The freeze point is the earliest `frozen_at_unix`. A freeze
-- point never predates its insertion by more than 60 s of clock skew, measured against
-- created_at, which the runtime role cannot set (stamp_created_at below); the CHECK alone does
-- not stop backdating, since created_at would otherwise be client supplied.
CREATE TABLE issuance_freezes (
  snapshot_hash bytea PRIMARY KEY CHECK (length(snapshot_hash) = 32),
  frozen_at_unix bigint NOT NULL
    CHECK (frozen_at_unix >= floor(extract(epoch FROM created_at))::bigint - 60),
  created_at timestamptz NOT NULL DEFAULT now()
);

-- Seal key of each stub, for the revocation backstop below. Required on new rows only (NOT VALID
-- skips pre-migration rows, which have none: no ServerKeyCert existed then). Pre-release lab, no
-- production data; stub_refusal refuses a cheque with no covering cert, so such a row is never paid.
ALTER TABLE cheques ADD COLUMN server_key_id bytea CHECK (length(server_key_id) = 16);
ALTER TABLE cheques ADD CONSTRAINT cheques_server_key_id_present
  CHECK (server_key_id IS NOT NULL) NOT VALID;

-- created_at feeds the freeze and cert-validity checks: the runtime role always gets now(), never
-- a client value. The owner (migrations, test setup) may still set it explicitly.
CREATE FUNCTION stamp_created_at() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF current_user = 'meshpay_app' THEN
    NEW.created_at := now();
  END IF;
  RETURN NEW;
END $$;

DO $$
DECLARE
  tbl text;
BEGIN
  FOREACH tbl IN ARRAY ARRAY['chequebooks', 'issuance_freezes', 'server_key_certs',
    'revoked_server_keys'] LOOP
    EXECUTE format(
      'CREATE TRIGGER %I BEFORE INSERT ON %I
       FOR EACH ROW EXECUTE FUNCTION stamp_created_at()',
      tbl || '_stamp_created_at', tbl);
  END LOOP;
END $$;

DO $$
DECLARE
  tbl text;
BEGIN
  FOREACH tbl IN ARRAY ARRAY['server_key_certs', 'revoked_server_keys', 'issuance_freezes'] LOOP
    EXECUTE format(
      'CREATE TRIGGER %I BEFORE UPDATE OR DELETE OR TRUNCATE ON %I
       FOR EACH STATEMENT EXECUTE FUNCTION forbid_mutation()',
      tbl || '_no_mutation', tbl);
  END LOOP;
END $$;

-- Database backstop of the kill switch: no chequebook or stub is written while frozen.
CREATE FUNCTION refuse_issuance_when_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF EXISTS (SELECT 1 FROM issuance_freezes) THEN
    RAISE EXCEPTION 'issuance frozen by kill switch' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER chequebooks_not_frozen BEFORE INSERT ON chequebooks
  FOR EACH ROW EXECUTE FUNCTION refuse_issuance_when_frozen();
CREATE TRIGGER cheques_not_frozen BEFORE INSERT ON cheques
  FOR EACH ROW EXECUTE FUNCTION refuse_issuance_when_frozen();

-- True when the cheque's chequebook was created after the issuance freeze point. Shared by the
-- redemption trigger and the deposit path.
CREATE FUNCTION issued_after_freeze(cheque bytea) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (
    SELECT 1 FROM cheques c JOIN chequebooks b USING (chequebook_id)
    WHERE c.cheque_id = cheque
      AND floor(extract(epoch FROM b.created_at))::bigint
        > (SELECT min(frozen_at_unix) FROM issuance_freezes))
$$;

-- A stub issued after the freeze point, or sealed by a revoked server key, is never paid.
CREATE FUNCTION refuse_redemption_after_freeze() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF issued_after_freeze(NEW.cheque_id) THEN
    RAISE EXCEPTION 'stub issued after the issuance freeze' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER redemptions_not_after_freeze BEFORE INSERT ON redemptions
  FOR EACH ROW EXECUTE FUNCTION refuse_redemption_after_freeze();

CREATE FUNCTION refuse_redemption_of_revoked_key() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM cheques c JOIN revoked_server_keys r ON r.key_id = c.server_key_id
    WHERE c.cheque_id = NEW.cheque_id
  ) THEN
    RAISE EXCEPTION 'stub sealed by a revoked server key' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;

CREATE TRIGGER redemptions_not_revoked_key BEFORE INSERT ON redemptions
  FOR EACH ROW EXECUTE FUNCTION refuse_redemption_of_revoked_key();

ALTER TABLE audit_log DROP CONSTRAINT audit_log_event_check;
ALTER TABLE audit_log ADD CONSTRAINT audit_log_event_check CHECK (event IN (
  'ACCOUNT_REGISTERED', 'CHEQUEBOOK_ISSUED', 'CHEQUE_REDEEMED', 'CHEQUE_RELEASED',
  'EQUIVOCATION_PROVED', 'SERVER_KEY_REVOKED', 'ISSUANCE_FROZEN'
));

-- Walks the chain from the genesis by links (not by seq: concurrent inserts may take seq out of
-- chain order). Valid only when every hash recomputes, the walk reaches every row and ends at
-- audit_head. Detects an altered, removed (first, middle or last) or reordered row; a rewrite of
-- the whole suffix including audit_head is caught only against a distributed audit_head_hash.
CREATE OR REPLACE FUNCTION audit_chain_valid() RETURNS boolean LANGUAGE sql STABLE AS $$
  WITH RECURSIVE walk (hash, n) AS (
    SELECT a.hash, 1 FROM audit_log a WHERE a.prev_hash = decode(repeat('00', 32), 'hex')
    UNION ALL
    SELECT a.hash, w.n + 1 FROM walk w JOIN audit_log a ON a.prev_hash = w.hash
    WHERE w.n <= (SELECT count(*) FROM audit_log) -- a tampered cycle cannot loop forever
  )
  SELECT NOT EXISTS (
      SELECT 1 FROM audit_log a
      WHERE a.hash <> sha256(a.prev_hash || convert_to(a.event, 'UTF8') || a.ref_id))
    AND (SELECT count(*) FROM walk) = (SELECT count(*) FROM audit_log)
    AND (SELECT hash FROM audit_head) = coalesce(
      (SELECT hash FROM walk ORDER BY n DESC LIMIT 1), decode(repeat('00', 32), 'hex'))
$$;

-- Tables and functions are owned by meshpay_owner (migrations run under SET ROLE meshpay_owner);
-- meshpay_app is not a member, so it cannot ALTER/DISABLE TRIGGER, DELETE or TRUNCATE them.
GRANT SELECT, INSERT ON server_key_certs, revoked_server_keys, issuance_freezes TO meshpay_app;
REVOKE ALL ON FUNCTION issued_after_freeze(bytea) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION issued_after_freeze(bytea) TO meshpay_app;
