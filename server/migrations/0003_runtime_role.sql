-- Least-privilege runtime role `meshpay_app` (created by `meshpay_server migrate` before this runs).
-- No DDL, no TRIGGER privilege, not superuser (cannot SET session_replication_role), and NO
-- INSERT on accounts, balances, supply, audit_head or ledger_entries: money moves only through
-- the SECURITY DEFINER post_* functions, whose amounts come from existing rows.
--
-- KNOWN LAB GAP: the runtime role still INSERTs the evidence rows (chequebooks, cheques,
-- redemptions, cheque_releases). Postgres cannot verify P-256 signatures, so a compromised
-- runtime role can forge mutually consistent evidence rows and move funds out of a victim's
-- reserve (it cannot mint beyond the grants nor exceed the victim's reserved balance). The
-- registration faucet (post_register) is also open to it, bounded by the immutable supply cap.
REVOKE ALL ON SCHEMA public FROM PUBLIC;
GRANT USAGE ON SCHEMA public TO meshpay_app;

GRANT SELECT ON accounts, balances, supply, audit_head, ledger_entries, chequebooks, cheques,
  cheque_releases, redemptions, equivocation_proofs, audit_log TO meshpay_app;
GRANT INSERT ON chequebooks, cheques, cheque_releases, redemptions, equivocation_proofs,
  audit_log TO meshpay_app;
GRANT UPDATE (frozen) ON accounts TO meshpay_app;

REVOKE ALL ON FUNCTION post_register(bytea), post_reserve(bytea), post_redeem(bytea),
  post_release(bytea), audit_chain_valid() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION post_register(bytea), post_reserve(bytea), post_redeem(bytea),
  post_release(bytea), audit_chain_valid() TO meshpay_app;
