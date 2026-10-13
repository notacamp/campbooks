---
name: add-module
description: Add a new domain module to Campbooks - check it is approved, scaffold it with `cargo xtask new-module`, then write its OpenSpec change and first slices. Use when a task introduces a new bounded context (a new crate under crates/modules), or when asked to create, add or scaffold a module.
---

A module is a bounded context: its own crate, Postgres schema, scopes,
events and client folder (AGENTS.md → Architecture rules). Adding one is an
architecture decision, so it has to be approved before it is built.

## 1. Check it belongs

- The module is listed in `docs/domain-map.md`, or an ADR approves it. If
  neither, stop: propose the domain-map entry (or an ADR) and ask.
- Its name is the one the domain map uses: one `snake_case` word if
  possible. It becomes the crate directory, the schema and the scope prefix
  (`<name>:read`), so it is hard to change later.
- It must not need another module's code. If it does, it needs events or an
  `architecture.toml` entry (reviewed like an ADR); say so in the proposal.

## 2. Scaffold

From the task's worktree:

```sh
cargo xtask new-module <name> --issue https://github.com/notacamp/campbooks-cloud/issues/<n>
cargo xtask openapi && pnpm --dir clients api:generate
```

This creates the crate (`scopes.rs` with `<name>:read`/`<name>:write`,
`events.rs`, an `overview` read slice with tests), the
`migrations/<ts>_<name>__init.sql` schema migration, the
`openspec/changes/add-<name>-module/` change, and the client module (with a
placeholder page and its four locale files), and registers the module in
`Cargo.toml`, the server and the client shell. It refuses to overwrite
anything that exists. Multi-word names use kebab-case on the client
(`clients/app/src/modules/mail-sync`), as the client's file-name lint wants.

## 3. Write the spec

Fill in every `TODO(new-module)`:

```sh
git grep -n 'TODO(new-module)'
```

Until the spec delta in the generated change states real requirements, it
fails `openspec validate --strict`, so the PR cannot merge unfinished. Also
translate the module's title in the `pt-PT`, `es-ES` and `fr` files.

## 4. Build it

Continue with the `develop-feature` skill from step 3: add slices with
`cargo xtask new-feature <name> <feature>`, replace the `overview`
placeholder (or delete it, with its route and spec scenario) once a real read
exists, add tables to the module's own schema with row-level security (the
comment in the generated migration shows how), then the gates, verification
and the PR. The PR carries a recording, since the module adds a page.
