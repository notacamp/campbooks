## 1. Keys

- [x] 1.1 `SecretKey` carries an optional previous key, read from `CAMPBOOKS_SECRET_KEY_PREVIOUS[_FILE]`, refused alone, equal or malformed
- [x] 1.2 `Cipher` opens with either key and says which (`open_checked`); `Signer` and `Binding` verify and open with either

## 2. Sealing again

- [x] 2.1 Identity signing keys sealed again when loaded
- [x] 2.2 Connection access tokens sealed again on use, refresh tokens at refresh
- [x] 2.3 Web session refresh tokens sealed again at refresh

## 3. Operations

- [x] 3.1 `init.sh rotate` / `forget-previous`, `.env.example`, the rotation runbook, a startup warning

## 4. Tests

- [x] 4.1 Kernel: open with the previous key, sealed again, the previous removed, wrong keys, env reading
- [x] 4.2 Identity signing keys and connection credentials across a rotation
