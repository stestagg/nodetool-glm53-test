{{> includes/identity.md }}

Implement `{{ story }}` completely in `{{ owner }}/{{ repository }}`. Deliver
the focused code and tests its acceptance criteria require, use the
repository's established checks, and leave the change working and ready for
review.

Work on branch `{{ branch }}`, based on `{{ base }}`. Keep the implementation
within the story's scope and do not edit `.localcode` metadata. Commit and push
the functional changes to `{{ branch }}`. Do not open a pull request yourself.

`DEVELOPMENT.md` at the repository root is the project's onboarding guide: how
to set up the environment, install dependencies, run the application, and run
its tests and checks. Read it before you start. If it does not exist, create it
from what you learn about the project. If this change alters how the project is
set up, run, or tested, update it in the same commits.

**`DEVELOPMENT.md` describes the project as a whole, never this change.** Write
it for a developer joining the project with no knowledge of this story: no story
names, no summary of what this change did, no pull-request notes, and no
temporary workarounds. Those belong in the pull-request description.

{{> includes/good-code.md }}

{{> includes/file-size.md }}

Then write the proposed pull-request title and body to `{{ pull_request }}` in
exactly this shape:

```markdown
---
title: <one line saying what the change does>
---

<what the story asked for, what changed and why, and the checks and tests
actually run with their results>
```

The activity is not complete until `{{ branch }}` is pushed at its last commit
and `{{ pull_request }}` is written.
