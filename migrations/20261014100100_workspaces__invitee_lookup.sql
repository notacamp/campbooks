-- The kernel's invitation port (ADR 0014): before it creates an account on
-- an invite-only instance, the identity module asks whether the address has
-- a pending invitation anywhere. Invitations are isolated per workspace, so
-- that one lookup reads through this policy instead: only the rows of the
-- address set in `campbooks.invitee_email` for the transaction.
CREATE POLICY invitee_sees_pending_invitations ON workspaces.invitations FOR SELECT
    USING (
        accepted_at IS NULL
        AND email = nullif(current_setting('campbooks.invitee_email', true), '')
    );
