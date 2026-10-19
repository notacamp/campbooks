# Proposal

## Why

`campbooks-server labelling export` selected by workspace only. In a shared
workspace, the sheet the operator copied off the server held mail from inboxes
their member cannot read: a teammate's private inbox. The folder also stayed
on the server until someone remembered to delete it, and a committed eval set
could claim `private` and skip the reserved-domain checks meant for real mail.
This blocks the owner labelling their actions (campbooks-cloud#234).

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/219

## What Changes

- Touched: `conversations` (the export), `workspaces` (finding a member for
  an operator command), the server's `labelling` CLI and `xtask eval`.
- `labelling export` requires `--member <id or verified address>` and exports
  only from the inboxes that member can read, through the same
  `visible_inboxes` rule the API uses; `--inbox <id>` narrows it to one of
  them and is refused (with a clear error) when the member cannot read it. It
  applies to every `--set`. Without `--member` the command is refused: there is
  no "whole workspace" export.
- The folder keeps its rules (a new folder only, `0700` / `0600`), carries a
  `.campbooks-labelling` marker, and the command prints its absolute path with
  a reminder to delete it. `labelling purge --out <folder>` removes a folder
  only when it carries that marker (never a link, never another folder). No
  automatic deletion: the owner needs time to label.
- `cargo xtask eval` refuses a set under `crates/ai/evals/` (a committed set)
  that claims `private: true`.

## Non-goals

- No change to what an item holds, the sheet's columns or the eval.
- No temporary folder deleted on exit.
