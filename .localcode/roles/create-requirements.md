{{> includes/identity.md }}

Record the requirements the design brief below states in `{{ requirements }}`.
If that file already exists, read it, add what the brief states that it does
not hold yet, and change an existing requirement only where the brief states it
differently. Otherwise, create it.

The requirements are a reference, not a design. Stories written later cite each
one by its id to show where it is met, and reviewers check stories against
them, so they must say what the brief asks for and nothing more:

- Extract every capability, behaviour, constraint, quality, and exclusion the
  brief states, keeping its meaning and, where it is precise, its wording.
- Write one requirement per bullet: split a statement that asks for two things,
  and record a repeated one once.
- Do not interpret, elaborate, prioritise, or fill gaps. Where the brief is
  vague or ambiguous, record it as the brief puts it rather than settling it;
  settling it is for the stories.
- Do not add requirements the brief does not state, however sensible, and do
  not describe how anything is to be built unless the brief itself requires it.

Work from the brief alone: there is no need to explore the repository. Group
the requirements under `## ` headings by area, so a reader can find their way
around, and write the file in exactly this shape:

```markdown
# Requirements

## <area>

- **REQ-1**: <one requirement, as the brief states it>
- **REQ-2**: <another>
```

Number requirements in order from `REQ-1`. An id names one requirement for
good: keep every existing id as it is, never renumber one, and number anything
new after the highest id already in the file.

Edit only `{{ requirements }}`. Do not implement anything, commit, push, or
change branches.

## Design brief

<design-brief>
{{ description }}
</design-brief>
