## Why

Replacing `CAMPBOOKS_SECRET_KEY` breaks everything it sealed. Everyone is
signed out, every connected mailbox needs reconnecting, and the signing keys
stop opening, which stops the server. So the key can never be rotated in
practice (campbooks-cloud#191, from the web BFF change's task 6.4).

## What Changes

- **The previous key.** `SecretKey` carries an optional previous key, read
  from `CAMPBOOKS_SECRET_KEY_PREVIOUS_FILE` (preferred) or
  `CAMPBOOKS_SECRET_KEY_PREVIOUS`. It is refused when it is set alone, equals
  the current key, or is malformed. `Debug` redacts both keys.
- **Opening and verifying.**
  - `Cipher` opens with the current key, then the previous one.
    `open_checked` says which key opened it (`Opened::by_previous`; its
    `Debug` hides the value).
  - `Signer` verifies and opens with either key.
  - `Binding` verifies with either key.
  - Sealing, signing and derived secrets use the current key only.
- **Sealing again on use.**
  - The identity module's signing keys are sealed again when loaded.
  - Connectors seal a connection's access token again when they use it, and
    its refresh token at the next refresh, even when the provider sends none
    back.
  - The BFF does the same for a session's refresh token at refresh.
  - Short-lived values open and expire: sign-ins, pending sign-ups, grants
    held for up to 7 days, and cursors.
- **Operations.**
  - `deploy/server/init.sh` gains `rotate`, which moves the key to
    `secret-key-previous` and generates a new one, and `forget-previous`.
  - The self-hosting guide documents the rotation: 30 days, then remove the
    previous key. It also says never to keep a leaked key as the previous
    one.
  - The server warns at startup while a previous key is set.

## Impact

- Spec: web-bff (new requirement: the instance secret key can be rotated).
- Code:
  - kernel `crypto` (`SecretKey`, `Cipher::open_checked`, `Opened`,
    `Signer`) and `binding`;
  - connectors `domain::credentials`;
  - identity `domain::keys`;
  - server `bff` and `main`.
- Deploy: `server/init.sh`, `.env.example`, README.
- No migration. Nothing changes until a previous key is set.
- Keys are not zeroized: the code never did, and this change keeps it so.
