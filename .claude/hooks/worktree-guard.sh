#!/usr/bin/env bash
# The worktree guard. A PreToolUse hook that keeps the main checkout read-only, so that each
# changeset is made on its own branch in its own worktree, and several agents can work in the
# repository at once.
#
# It denies:
#   - an Edit, a Write, or a NotebookEdit of a file inside the main checkout;
#   - a Bash command that runs in the main checkout and writes to git: commit, merge, rebase,
#     cherry-pick, revert, am, switch, checkout, push, reset, and stash.
# It permits each read, each command in a worktree, and each git command that does not
# write: status, log, diff, fetch, pull, branch, worktree.
#
# Input: the hook JSON on stdin. Output: a permission decision as JSON, or nothing.
set -u

input=$(cat)
tool=$(printf '%s' "$input" | jq -r '.tool_name // empty')
cwd=$(printf '%s' "$input" | jq -r '.cwd // empty')

# The main checkout is the directory that holds the .git directory. In a worktree, the
# git-dir is a file and the common dir points back to the main checkout.
main_root() {
  local dir="$1"
  local common
  common=$(git -C "$dir" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || return 1
  dirname "$common"
}

in_main() {
  # True when the path is inside the main checkout and not inside a linked worktree.
  local path="$1" dir root
  dir="$path"
  [ -d "$dir" ] || dir=$(dirname "$path")
  root=$(main_root "$dir") || return 1
  case "$path" in
    "$root"|"$root"/*) ;;
    *) return 1 ;;
  esac
  # A linked worktree can live under the main checkout. Exclude each one.
  local wt
  while read -r wt; do
    [ "$wt" = "$root" ] && continue
    case "$path" in
      "$wt"|"$wt"/*) return 1 ;;
    esac
  done < <(git -C "$root" worktree list --porcelain 2>/dev/null | sed -n 's/^worktree //p')
  return 0
}

deny() {
  jq -n --arg reason "$1" '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: $reason
    }
  }'
  exit 0
}

case "$tool" in
  Edit|Write|NotebookEdit)
    file=$(printf '%s' "$input" | jq -r '.tool_input.file_path // .tool_input.notebook_path // empty')
    [ -n "$file" ] || exit 0
    case "$file" in /*) ;; *) file="$cwd/$file" ;; esac
    if in_main "$file"; then
      deny "The file is inside the main checkout, which is read-only. Make the change on a branch in a worktree: git worktree add ../SnapshotPB-Manual-wt/<branch> -b <branch> origin/main. See the section Branches and worktrees in CLAUDE.md."
    fi
    ;;
  Bash)
    command=$(printf '%s' "$input" | jq -r '.tool_input.command // empty')
    [ -n "$command" ] || exit 0
    # A git command that writes, anywhere in the command line.
    if printf '%s' "$command" | grep -Eq '(^|[;&|(]|[[:space:]])git([[:space:]]+-[A-Za-z-]+([[:space:]]+[^[:space:]]+)?)*[[:space:]]+(commit|merge|rebase|cherry-pick|revert|am|switch|checkout|push|reset|stash)([[:space:]]|$)'; then
      reason="The git command writes in the main checkout, which is read-only. Commit, merge, and push on a branch in a worktree under ../SnapshotPB-Manual-wt/. See the section Branches and worktrees in CLAUDE.md."
      # `git -C <path>` names where the command acts.
      target=$(printf '%s' "$command" | sed -n 's/.*git[[:space:]]\+-C[[:space:]]\+\([^[:space:]]\+\).*/\1/p' | head -n 1)
      if [ -n "$target" ]; then
        case "$target" in /*) ;; *) target="$cwd/$target" ;; esac
        if in_main "$target"; then deny "$reason"; fi
        exit 0
      fi
      # A command that names a worktree path acts there, not in the main checkout.
      if printf '%s' "$command" | grep -q 'SnapshotPB-Manual-wt/'; then
        exit 0
      fi
      [ -n "$cwd" ] || exit 0
      if in_main "$cwd"; then deny "$reason"; fi
    fi
    ;;
esac
exit 0
