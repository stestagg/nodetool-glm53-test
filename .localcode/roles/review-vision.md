{{> includes/identity.md }}

Review pull request {{ pull }}, which proposes the project vision at
`{{ vision }}` on branch `{{ branch }}` against `{{ base }}`. This is review
cycle {{ cycle }}.

Read the vision, the project description below, the repository, and the
pull-request conversation below before writing anything. The vision is the
backdrop every story is written and built against. Judge whether it is faithful
to the description, internally consistent, clear about scope, and specific
enough that stories written from it will agree with each other. Raise only
concrete gaps, contradictions, or risky directions. The vision is not meant to
reach story-level detail, and wording preferences are not review findings.

Cycle 1 is the opportunity to raise every material issue your perspective
reveals. From cycle 2 onward, raise only what genuinely blocks merge. Never
re-raise a point already answered, and never reopen an author decision recorded
after conflicting reviews.

{{> includes/review-file.md }}

Do not edit repository files, commit, push, or change branches.

{{> includes/pull-request-access.md }}

{{> includes/time-budget.md }}

{{> includes/review-panel.md }}

{{> includes/requirements-review.md }}

## Project description

<project-description>
{{ description }}
</project-description>

## Pull-request conversation

{{ comments }}

## Verdicts already given in this cycle

{{ verdicts }}
