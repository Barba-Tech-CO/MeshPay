-- Monotonic `RevocationSnapshot.version`: DTN peers keep a snapshot only when its version is
-- strictly greater than the held one (core PV-DTN-04), so a clock step back or two snapshots in
-- one second must still yield a new, larger version. One row; meshpay_app bumps it only through
-- next_snapshot_version, never writes it directly.
CREATE TABLE snapshot_version (
  singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
  last_version bigint NOT NULL CHECK (last_version >= 0)
);
INSERT INTO snapshot_version (last_version) VALUES (0);

CREATE FUNCTION next_snapshot_version(now_unix bigint) RETURNS bigint LANGUAGE sql
  SECURITY DEFINER SET search_path = public, pg_temp AS $$
  UPDATE snapshot_version SET last_version = GREATEST(now_unix, last_version + 1)
  RETURNING last_version;
$$;

REVOKE ALL ON FUNCTION next_snapshot_version(bigint) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION next_snapshot_version(bigint) TO meshpay_app;
