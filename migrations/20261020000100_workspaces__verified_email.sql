-- Whether the identity provider verified a user's address. Other members
-- see a member's address only when it was verified, so nobody appears in a
-- workspace under an address they never proved. Unknown for existing users
-- (false) until their next sign-in says.
ALTER TABLE workspaces.users ADD COLUMN email_verified boolean NOT NULL DEFAULT false;
