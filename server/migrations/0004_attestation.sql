-- Attestation level decided by server-side verification (PRD 4.3 to 4.5), plus single-use
-- registration challenges. lab_software exists only for LAB_MODE; the others come from a verified
-- Android Key Attestation chain.
CREATE TYPE attestation_level AS ENUM ('lab_software', 'software_attested', 'tee', 'strongbox');

ALTER TABLE accounts DROP CONSTRAINT accounts_attestation_level_check;
ALTER TABLE accounts ALTER COLUMN attestation_level TYPE attestation_level
  USING lower(attestation_level)::attestation_level;

DROP FUNCTION post_register(bytea);
CREATE FUNCTION post_register(p_public_key bytea, p_level attestation_level) RETURNS boolean
  LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
  inserted bigint;
BEGIN
  INSERT INTO accounts (account_id, public_key, attestation_level)
  VALUES (substring(sha256(p_public_key) FROM 1 FOR 16), p_public_key, p_level)
  ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS inserted = ROW_COUNT;
  RETURN inserted = 1;
END $$;

REVOKE ALL ON FUNCTION post_register(bytea, attestation_level) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION post_register(bytea, attestation_level) TO meshpay_app;

-- Server-issued random nonce; the attestation challenge must equal it. Deleted on first use.
CREATE TABLE challenges (
  nonce bytea PRIMARY KEY CHECK (length(nonce) = 32),
  expires_at_unix bigint NOT NULL
);
GRANT SELECT, INSERT, DELETE ON challenges TO meshpay_app;
CREATE INDEX challenges_expires_at_idx ON challenges (expires_at_unix);
