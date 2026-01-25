# Task Workflow

How we iterate on work in this project.

## Overview

1. **Discussion** - Talk through the feature/task, ask questions, clarify requirements
2. **Documentation** - When clear, write a dedicated `docs/tasks/<name>.md` file
3. **Implementation** - Go phase by phase, with review after each phase
4. **Commit** - After each phase is approved, commit and move on

## Task Document Structure

Each task doc in `docs/tasks/` should have:

- Overview of what we're building
- Design decisions and rationale
- **Relevant files/directories** - Link to key files the LLM will need to read or modify. This helps the LLM get up to speed quickly when starting or resuming work.
- Phases broken into actionable items with checkboxes
- **Future Work section** - A place to capture ideas and improvements discovered during implementation (see below)

Example:

```markdown
## Relevant Files

- `crates/host/src/cli.rs` - CLI definitions
- `crates/host/src/main.rs` - Command dispatch
- `bench-results/` - Benchmark output directory

## Phases

### Phase 1: CLI scaffolding

- [ ] Add subcommand to cli.rs
- [ ] Handle command in main.rs (placeholder)
- [ ] Verify it compiles

### Phase 2: Core logic

- [ ] Create new module
- [ ] Implement data loading
- [ ] Add tests

## Future Work

- (items added here as we discover them during implementation)
```

## Phase Workflow

1. **LLM implements** - Work through the checkboxes for the current phase
2. **Human reviews** - Look at the changes, ask for tweaks if needed
3. **Iterate** - Until both parties are happy
4. **Check off** - Mark all checkboxes complete for that phase
5. **Commit** - LLM proposes a commit message, human approves or tweaks, then commit
6. **Next phase** - Move directly to the next phase

## Future Work Section

During implementation, we often discover things that aren't ideal but are out of scope for the current task. Instead of losing these insights, add them to the **Future Work** section at the bottom of the task document.

**Use checkboxes** for Future Work items too, so we can track which ones have been addressed later.

It's the human's responsibility to notice these during review and suggest items to add. Examples:
- [ ] "This function is getting long, could be refactored later"
- [ ] "We're duplicating logic with X, could consolidate"
- [ ] "Would be nice to add Y feature eventually"

## Resuming Work

If we pause mid-task, the checkboxes show exactly where we left off. To resume:

1. Open the relevant `docs/tasks/<name>.md`
2. Find the first unchecked item
3. Continue from there
