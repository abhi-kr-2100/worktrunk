function git-worktree-merge -d "Finish worktree and merge branch into target branch (default: main)"
    argparse -n git-worktree-merge s/squash k/no-remove -- $argv
    or return 1

    # Get current branch
    set -l branch (git branch --show-current)
    if test -z "$branch"
        echo (set_color red)"❌ Failed to determine current branch"(set_color normal) >&2
        return 1
    end

    # Get target branch
    set -l target_branch $argv[1]
    if test -z "$target_branch"
        set target_branch (git default-branch)
        or return 1
    end

    # Handle uncommitted changes (including untracked files)
    if __git_is_dirty
        # Warn if user had staged changes that will be mixed in
        if not git diff --cached --quiet --exit-code
            echo (set_color yellow)"💡 Note: Adding all changes (you had some staged changes)"(set_color normal) >&2
        end

        if set -ql _flag_squash
            # When squashing, just stage changes - they'll be included in the squash
            git add .
        else
            # When not squashing, commit changes normally
            git add .
            # Check if there's actually anything to commit after staging
            if not git diff --cached --quiet --exit-code
                echo (set_color cyan)"🔄 Committing uncommitted changes..."(set_color normal) >&2
                if not git-commit-llm
                    echo (set_color red)"❌ Failed to commit changes"(set_color normal) >&2
                    return 1
                end
            end
        end
    end

    # Check primary worktree state if we're in a worktree
    set -l git_dir (git rev-parse --git-dir)
    set -l common_dir (git rev-parse --git-common-dir)

    if test "$git_dir" != "$common_dir"
        set -l primary_worktree_dir (dirname $common_dir)

        if set -l operation (fish -c "cd '$primary_worktree_dir' && git-is-operation-in-progress")
            echo (set_color red)"❌ Primary worktree has a $operation in progress"(set_color normal) >&2
            git -C "$primary_worktree_dir" status --short
            return 1
        end
    end

    # Prepare commits
    if set -ql _flag_squash
        if not git-squash --rebase "$target_branch"
            echo (set_color red)"❌ Failed to squash and rebase commits"(set_color normal) >&2
            return 1
        end
    else
        if not git rebase "$target_branch"
            echo (set_color red)"❌ Failed to rebase onto "(set_color red --bold)"$target_branch"(set_color normal)(set_color red)" "(set_color normal) >&2
            return 1
        end
    end

    # Check if we're already on the target branch
    if test "$branch" = "$target_branch"
        echo (set_color -d)"💬 Already on "(set_color -d --bold)"$target_branch"(set_color normal -d)", nothing to merge"(set_color normal) >&2
        return 0
    end

    # Fast-forward target branch
    if not git-worktree-push "$target_branch"
        echo (set_color red)"❌ Failed to fast-forward "(set_color red --bold)"$target_branch"(set_color normal)(set_color red)" "(set_color normal) >&2
        return 1
    end

    # Finish worktree unless --no-remove was specified
    if not set -q _flag_no_remove
        # Always finish the current worktree first (handles cleanup)
        if not git-worktree-finish
            echo (set_color red)"❌ Failed to finish worktree"(set_color normal) >&2
            return 1
        end

        # Now try to switch to the target branch
        set -l current_branch (git branch --show-current)
        if test "$current_branch" != "$target_branch"
            echo (set_color cyan)"🔄 Switching to $target_branch..."(set_color normal) >&2
            # Try regular switch first
            if not git switch "$target_branch" 2>/dev/null
                # If switch fails, target branch is likely in another worktree
                # Use git-worktree-switch to handle that case
                git-worktree-switch "$target_branch"
            end
        end
    else
        echo (set_color green)"✅ Branch merged to $target_branch (worktree preserved)"(set_color normal) >&2
    end
end
