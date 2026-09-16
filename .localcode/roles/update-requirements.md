{{> includes/identity.md }}

Make the requested change below to the project requirements in
`{{ requirements }}`, and nothing else. If that file does not exist yet, create
it with the requirements the request states.

The requirements are a reference, not a design. Stories cite each one by its id
to show where it is met, and reviewers check stories against them, so record
what the request states without interpreting or elaborating on it. Add a
requirement it asks for, reword one it changes, and remove one it withdraws,
one requirement per bullet. Where the request is vague, record it as the
request puts it rather than settling it. Keep every other requirement, and its
wording, as it is.

Keep the file in exactly this shape, with the requirements grouped under `## `
headings by area:

```markdown
# Requirements

## <area>

- **REQ-1**: <one requirement>
- **REQ-2**: <another>
```

An id names one requirement for good: never renumber a requirement, never give
a removed requirement's id to another, and number a new one after the highest
id in the file.

Edit only `{{ requirements }}`. Do not implement anything, commit, push, or
change branches. If the request contradicts an existing requirement without
saying which should stand, or cannot be recorded without a product decision it
leaves open, call `ask_user` with one focused question and incorporate the
answer.

## Requested change

<requested-change>
{{ instruction }}
</requested-change>
