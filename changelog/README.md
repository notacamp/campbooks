# Changelog entries

Each change waiting for a release is **one file**, so PRs that run in parallel
never edit the same lines (one shared `CHANGELOG.md` made almost every PR
conflict with every other one):

```
changelog/unreleased/<kind>/<slug>.md
```

- **kind** is the folder: `added`, `changed`, `deprecated`, `removed`, `fixed`
  or `security` (the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
  headings).
- **slug** is the issue and a few words, lowercase with dashes:
  `240-sent-view.md`, `fix-send-after-edit.md`.
- The file holds one list item (or a few, when one PR adds several things), in
  the style of `CHANGELOG.md`: a bold sentence saying what changed, then what a
  user or self-hoster notices. No headings; the release adds them.

```md
- **Sent, Archive, Spam and Trash under the streams** (G then 5 to 8). Sent lists every
  conversation you wrote in, by your latest message, with whom it went to.
```

A PR that fixes one thing and adds another adds one file in `fixed/` and one in
`added/`. Start one with:

```sh
.github/scripts/changelog.sh new added 240-sent-view   # prints the path to edit
.github/scripts/changelog.sh check                      # what the Policy check runs
.github/scripts/changelog.sh preview                    # the next release's notes so far
```

`feat` and `fix` PRs (by branch or title) must add an entry (the Policy check fails them
otherwise), and **no PR edits `CHANGELOG.md` directly**: it is written only by
`changelog.sh release`, which gathers every entry into a dated
`## [next-vX.Y.Z]` section, deletes the files and bumps the version
(CONTRIBUTING.md → Releases).

Every green merge to `next` is deployed, so an entry here describes something
already live; the release only gives it a version.
