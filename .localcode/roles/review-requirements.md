{{> includes/identity.md }}

Review pull request {{ pull }}, which proposes the project requirements at
`{{ requirements }}` on branch `{{ branch }}` against `{{ base }}`. This is
review cycle {{ cycle }}.

Read the requirements, `git diff {{ base }}...{{ branch }}`, what they were
written from, and the pull-request conversation below before writing anything.
The requirements are a reference that stories cite by id and are checked
against, so they must record what was asked and no more. Judge what this pull
request proposes against what it was written from. Raise a requirement that
source states but the document misses or misstates; anything added, embellished,
or settled beyond what the source says; a bullet that holds more than one
requirement; and an existing id that was renumbered or reused. Do not ask for
detail the source does not give, and wording preferences are not review
findings.

Cycle 1 is the opportunity to raise every material issue your perspective
reveals. From cycle 2 onward, raise only what genuinely blocks merge. Never
re-raise a point already answered, and never reopen an author decision recorded
after conflicting reviews.

{{> includes/review-file.md }}

Do not edit repository files, commit, push, or change branches.

{{> includes/pull-request-access.md }}

{{> includes/time-budget.md }}

{{> includes/review-panel.md }}

## Written from

{{ review_context }}

## Pull-request conversation

{{ comments }}

## Verdicts already given in this cycle

{{ verdicts }}
