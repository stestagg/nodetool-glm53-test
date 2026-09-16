{{> includes/identity.md }}

The backlog has been written out. Trace the project's requirements in
`{{ requirements }}` to the stories that meet them, and add stories for
whatever the stories leave out.

Read the requirements, then every story listed below, at every stage. For each
requirement, find the stories that meet it: those that cite its id, and those
that plainly meet it without citing it. A requirement is met only when the
stories together meet all of it, as it is stated. A story that meets only part
of a requirement leaves the rest of it missing, and a story that contradicts a
requirement does not meet it.

For whatever is missing, add new placeholder stories under
`.localcode/stories/backlog/`, numbered from `{{ first }}`, with slug-style
names, each in exactly the shape of the placeholder template below and keeping
`placeholder: true`. Give each a title and a one-line summary of the outcome
that delivers the missing part, citing the ids it covers -- `(REQ-4)`, say. Each
one is written out in full, reviewed, and built later, like any other story, so
do not write it out now. Group missing parts that belong together into one
story, add at most {{ new_limit }}, and add nothing for a requirement the
stories already meet. Leave every existing story exactly as it is.

Then write `{{ trace }}` with one line for every requirement, each exactly once,
in exactly this shape:

```markdown
- REQ-<n> | met | <story file name>, <story file name>
- REQ-<n> | added | <story file name you added>, <existing story file name>
```

`met` names the existing stories that meet the requirement. `added` names the
stories you added for what was missing, and any existing stories that meet the
rest of it. Name stories by file name, as `03-print-a-fortune.md`. Do not treat
anything the requirements document does not state as a requirement.

Add only new placeholder stories, and write `{{ trace }}`. Do not edit existing
stories, implement anything, commit, push, or change branches.

## Stories by stage

{{ stories }}

## Placeholder template

{{ template }}

## Story repository guidance

{{ guide }}
