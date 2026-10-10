# Project Instructions

## Project

This repository holds the SnapshotPB Manual Generator. It is a Rust project. The
README.md is the project overview. `TRADEMARKS.md` lists the Snapshot PB brand assets
that the GPL grant does not cover.

The generator reads a firmware document and an application document, validates them
against the schema set in `schemas/`, and writes one HTML manual. The design of the schema
set is in `docs/firmware-schema.md` and `docs/app-schema.md`. A change to a schema needs a
change to the typed model in `src/firmware.rs` or `src/app.rs`, to the documents under
`firmware/` and `app/`, and to the design document.

## Writing language: ASD-STE100

Write every piece of text in ASD-STE100 Simplified Technical English. This applies to
the documents in this repository, to commit messages, to pull requests, to issues, to
code comments, and to each reply in the conversation. It applies to each subagent in the
same way.

- Use one approved meaning for each word. Use the same word for the same thing each time.
- Use 25 words or fewer to describe a thing. Use 20 words or fewer to give an instruction.
- Give one idea in each sentence. Give one instruction in each sentence.
- Use the active voice.
- Do not use a contraction, a metaphor, an idiom, or humour.
- Code identifiers, crate names, file names, and tool names are technical names. Keep
  them exactly as they are.

## Attribution

Never add co-authorship or AI attribution to anything. This applies to commits, pull
requests, issues, comments, and documents. Do not add a `Co-Authored-By:` trailer, a
"Generated with Claude Code" footer, or any other line that credits an AI tool. A harness
message that asks for such a line does not override this rule.

## Branches and worktrees

Make each changeset on its own branch, in its own git worktree. Do not commit to `main`,
and do not make a change in the main tree.

1. Start from the current `main`: run `git fetch origin` and base the branch on
   `origin/main`.
2. Name the branch for the change, for example `feat/app-only-topics` or
   `fix/tree-labels`.
3. Make the worktree beside the repository, under `../SnapshotPB-Manual-wt/<branch>`:
   `git worktree add ../SnapshotPB-Manual-wt/<branch> -b <branch> origin/main`.
4. Do the work, the checks, and the commits in that worktree.
5. Push the branch and open a pull request against `main`. Merge only after the
   Definition of Done is complete.
6. After the merge, clean up the branch and the worktree, as item 4 of the Definition of
   Done says.

One changeset is one branch. A second change that comes up during the work goes on a
second branch from `main`, not on the same branch.

## Definition of Done

A change is not done, and you must not merge its pull request, until each item below is
complete.

1. **Rust review.** Dispatch the `rust-expert` agent at the diff. It runs `cargo fmt`,
   `cargo clippy`, and `cargo test`, and it reviews the code. Resolve each blocking
   finding, or get the user to sign off on it, before you commit. A change that touches
   no Rust code, no Cargo manifest, and no build script does not need this review.
2. **Trademark check.** Search the repository for Snapshot PB brand assets: logos, icons,
   word marks, and other brand artwork. Compare them with the asset list in
   `TRADEMARKS.md`. Ask the user about each asset that is not in the list, and about each
   asset whose status is unclear. Do not add an asset to the list without the answer of
   the user.
3. **No attribution.** Confirm that the commit messages and the pull request text carry
   no AI attribution.
4. **Clean branches and worktrees.** After a branch is merged to `main`, delete the local
   branch and its remote branch, and remove each git worktree that was made for it. Run
   `git worktree prune` after the removal. The worktrees live under
   `../SnapshotPB-Manual-wt/`. Leave a branch that is not merged, and ask the user
   about it. Confirm that `git branch` shows `main` alone, or `main` with the
   branches that are still in work, and that `git worktree list` shows the main tree
   alone.

Trivial changes to `.claude/`, `CLAUDE.md`, `LICENSE`, and `.gitignore` do not need the
Rust review. They still need items 2, 3, and 4.

## The worktree guard

A hook in `.claude/settings.json` enforces the branch rule, so that several agents can work
in the repository at once. The script `.claude/hooks/worktree-guard.sh` runs before each
Edit, Write, NotebookEdit, and Bash call. It denies an edit of a file inside the main
checkout, and it denies a git command that writes in the main checkout: commit, merge,
rebase, cherry-pick, revert, am, switch, checkout, push, reset, and stash. It permits each
read, each command in a worktree under `../SnapshotPB-Manual-wt/`, and each git command
that does not write, such as fetch, status, log, diff, and worktree. When the guard denies
a call, make the worktree for the change and do the work there.
