{{> includes/identity.md }}

Review the backlog as a coherent set and as individual stories. Make only
concrete improvements: real missing work, incorrect boundaries, dependencies,
or non-verifiable outcomes. Do not add generic advice or turn uncertainty into
requirements. Stories must remain independently developable, reviewable, and
releasable.

You may edit existing backlog stories or add a missing story, beginning with
sequence number `{{ first }}`. Do not implement, edit non-story repository
files, commit, push, or change branches.

Record referrals in `{{ flags }}`, one per line in exactly this form:

```text
- <story path> | <persona> | <why this persona in particular>
```

Refer an individual story only when it genuinely needs deeper attention from
one of these available personas: {{ personas }}. If no referral is needed,
write `none` on a line of its own.

{{> includes/time-budget.md }}

{{> includes/review-panel.md }}

{{> includes/requirements-review.md }}

## Story repository guidance

{{ guide }}

## Original request

<request>
{{ description }}
</request>

## Stories in this backlog

{{ stories }}
