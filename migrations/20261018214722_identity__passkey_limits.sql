-- Bounds on what passkey ceremonies store from unauthenticated requests
-- (security review of #465). The server already checks every value before
-- inserting it; these make an oversized row impossible whatever the code
-- does, so no request can grow the table beyond a few hundred bytes a row.
--
-- request_id: an authorization request id (issued ones are 43 characters).
-- display_name: a cleaned name, at most 64 characters.
-- return_to: a same-origin path, at most 2048 bytes (as everywhere else).
ALTER TABLE identity.webauthn_challenges
    ADD CONSTRAINT webauthn_challenges_request_id_len
        CHECK (request_id IS NULL OR octet_length(request_id) BETWEEN 1 AND 64),
    ADD CONSTRAINT webauthn_challenges_display_name_len
        CHECK (display_name IS NULL OR char_length(display_name) BETWEEN 1 AND 64),
    ADD CONSTRAINT webauthn_challenges_return_to_len
        CHECK (return_to IS NULL OR octet_length(return_to) BETWEEN 1 AND 2048);

-- A COSE key of the accepted algorithms is under 1.1 KiB (RSA-8192); the
-- transports are a de-duplicated subset of six known names.
ALTER TABLE identity.passkeys
    ADD CONSTRAINT passkeys_public_key_len CHECK (octet_length(public_key) <= 2048),
    ADD CONSTRAINT passkeys_transports_len CHECK (cardinality(transports) <= 6);
