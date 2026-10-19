- **The architecture check gives the same answer on every run.** `cargo xtask arch` could
  pick a doc comment that quotes an operation's declaration as that operation's handler,
  depending on the order the files were read, and fail at random. It now reads only code,
  prefers a real declaration over a listed id, and walks the files in a fixed order. A
  central-check call that is commented out no longer counts.
