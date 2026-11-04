-- Typed reference data for attestation levels and cap profiles (PV-LED-04).
CREATE TABLE attestation_levels (
  level attestation_level PRIMARY KEY
);

INSERT INTO attestation_levels (level) VALUES
  ('lab_software'),
  ('software_attested'),
  ('tee'),
  ('strongbox');

ALTER TABLE accounts
  ADD CONSTRAINT accounts_attestation_level_fk
  FOREIGN KEY (attestation_level) REFERENCES attestation_levels (level);

CREATE TABLE cap_profiles (
  profile_id text PRIMARY KEY CHECK (length(profile_id) > 0),
  attestation_level attestation_level NOT NULL REFERENCES attestation_levels (level),
  reserve_units bigint NOT NULL CHECK (reserve_units > 0),
  max_cheque_units bigint NOT NULL CHECK (max_cheque_units > 0),
  expires_seconds bigint NOT NULL CHECK (expires_seconds > 0),
  multiplier double precision NOT NULL CHECK (multiplier BETWEEN 0 AND 1),
  max_cheque_count integer NOT NULL CHECK (max_cheque_count BETWEEN 1 AND 10),
  CONSTRAINT cap_profile_face_within_reserve CHECK (max_cheque_units <= reserve_units),
  CONSTRAINT cap_profile_count_within_limit CHECK (
    reserve_units::numeric <= max_cheque_units::numeric * max_cheque_count::numeric
  )
);

GRANT SELECT ON attestation_levels, cap_profiles TO meshpay_app;
