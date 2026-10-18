## 1. Desktop shell

- [x] 1.1 `tauri.localhost` only on Windows and Android, tested per platform
- [x] 1.2 Mail links kept percent-encoded (`%20`, CR/LF), `attach=` dropped, tested
- [x] 1.3 Dev server only in a debug build, inside the tested function

## 2. WebKit check

- [x] 2.1 Sandbox and referrer policy from `frame.ts`
- [x] 2.2 Positive control: `allow-scripts` and no policy sets the flag
