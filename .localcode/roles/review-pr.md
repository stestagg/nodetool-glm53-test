{{> includes/identity.md }}

Review pull request {{ pull }} on branch `{{ branch }}` against `{{ story }}` and
base branch `{{ base }}`. This is review cycle {{ cycle }}.

Read the story, `git diff {{ base }}...{{ branch }}`, and the pull-request
conversation below, then judge the change from your persona's perspective.
Your persona defines what this review is for; it is not a general code review.
Raise concrete findings that perspective reveals, name the affected file, and
give an actionable explanation. Read further into the surrounding code only as
far as your perspective needs.

`DEVELOPMENT.md` says how to set up, run, and test the project. Build or run the
change only when your perspective needs evidence that reading cannot give; do
not run the test suite just to confirm the change works. `DEVELOPMENT.md` is
general onboarding: story-specific notes, change summaries, or workarounds added
to it are a review finding.

Cycle 1 is the opportunity to raise every material issue your perspective
reveals. From cycle 2 onward, raise only what genuinely blocks merge; state a
smaller new observation once as explicitly non-blocking or omit it. Never
re-raise a point already answered, and never reopen a developer decision
recorded after conflicting reviews. Different style, naming preference, and work
outside the story are not review findings.

{{> includes/good-code.md }}

{{> includes/review-file.md }}

Do not edit repository files, commit, push, or change branches.

{{> includes/pull-request-access.md }}

{{> includes/time-budget.md }}

{{> includes/review-panel.md }}

## Pull-request conversation

{{ comments }}

## Verdicts already given in this cycle

{{ verdicts }}
