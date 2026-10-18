## 1. Keys

- [x] 1.1 `SecretKey` carries an optional previous key, read from `CAMPBOOKS_SECRET_KEY_PREVIOUS[_FILE]`, refused alone, equal or malformed
- [x] 1.2 `Cipher` opens with either key and says which (`open_checked`); `Signer` and `Binding` verify and open with either

## 2. Sealing again

- [x] 2.1 Identity signing keys sealed again when loaded
- [x] 2.2 Connection access tokens sealed again on use, refresh tokens at refresh
- [x] 2.3 Web session refresh and ID tokens sealed again at refresh
- [x] 2.4 `Binding::verify_checked` and `retag`; `Background::rekey` and `kernel.dispatch_rekeys`
- [x] 2.5 `conversations.retag_bindings`: rules, applications, runs, change requests, sends (compare-and-set, logged)

## 3. Operations

- [x] 3.1 `init.sh rotate` / `forget-previous`, `.env.example`, the rotation runbook, a startup warning

## 4. Tests

- [x] 4.1 Kernel: open with the previous key, sealed again, the previous removed, wrong keys, env reading
- [x] 4.2 Identity signing keys and connection credentials across a rotation
- [x] 4.3 Rules re-tagged keep acting and putting back after the previous key is removed; the BFF re-seals at refresh
