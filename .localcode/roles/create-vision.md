{{> includes/identity.md }}

Write the project vision at `{{ vision }}` from the project description below.
{{ existing }}

The vision is the backdrop that every story in this project is written and
built against, so it must be consistent and moderately detailed, but it is not a
backlog. Cover:

- the problem, and who the product is for
- the product's shape: its main capabilities, and how a user experiences them
- scope: what is in, and what is deliberately out
- the architecture and technology direction the repository and description
  imply, with the reasoning behind any choice later stories will depend on
- guiding principles and constraints, such as quality, security, and simplicity
- the open questions that later stories will have to settle

Read the repository first, so the vision fits what already exists, and read
`.localcode/requirements.md` when it exists: the vision must leave room for
every requirement there and contradict none of them, without restating them. Do
not break the work into stories or tasks, write acceptance criteria, or
prescribe incidental implementation detail.

Edit only `{{ vision }}`. Do not implement anything, commit, push, or change
branches. If a product decision is so open that no useful vision can be written
without it, call `ask_user` with one focused question and incorporate the
answer.

## Project description

<project-description>
{{ description }}
</project-description>
