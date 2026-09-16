# Good code

Guidance for writing and reviewing code in this project.

Good code shares many principles with well-written textbooks:

- Presents information clearly and logically, making it easy to follow and understand.
- Balances vagueness with pedantry (`fn do_stuff()` is bad, but so is `fn
  add_one_to_integer_field_foo_raising_on_negative_values()`). This is a story-telling issue: work out what matters to
  an engaged reader to help them understand the intent.
- Doesn't over-compress information to the point where it becomes difficult to understand.
- Takes the reader on a logical journey, with self-consistent statements following on from each other.
- Sometimes abstractions are needed and important, but they must be true, non-leaky, and actually provide value.

## Design

- Code should tell a story (abstraction). Know the larger narrative the code models, and keep that story consistent and
  on-point. Different areas of the codebase may have different narratives.
- Keep abstractions generic. Detail that serves one caller or use case does not belong in a shared model. But also avoid
  exposing complexity as a way to inject into a 'generic' model. If you end up injecting so many parameters into an
  abstract model, then it has probably failed as an abstraction: `animal.walk(num_feet=2, joint_model=..,
  energy_source=.., terrain=..)` is a sign of a model going off the rails.
- Write the simplest, most direct solution to the problem at hand: not heroic, clever, or over-engineered. Minimal
  design is still design; it stays comprehensive and focused.
- Keep concerns separate. Logic outside a unit's core responsibility moves to where it belongs.
- Build for the change in front of you, shaped so likely future requirements need little new code. Flexibility nobody
  has asked for is complexity.
- Tell a tactical fix from a spreading workaround. A niche fix with no long-term consequence is fine; one that invites
  more workarounds is not. A niche fix that has been extended or worked around is a strong signal that this fix needs
  addressing properly.

## Size and duplication

- Prefer less code. Before adding, ask what can be removed or written in fewer lines.
- A simplification or refactor makes the codebase smaller, not larger.
- Remove duplicated, parallel, or nearly parallel code paths, and legacy or unused functions, when a small change allows
  it without bending an abstraction.
- Avoid migration paths, legacy handlers, and backwards-compatibility code. Weigh them against the project's stage; a
  one-off migration often beats a permanent second code path.
- Compatibility code that must stay is marked deprecated, with the condition under which it can be removed.
- A normal code file stays under 1000 lines and never exceeds 2000. Past either limit, split cohesive parts into their
  own modules, and move tests out of source files. Generated files, lockfiles, and data files are exempt.
- Default to 120 characters per line, but sometimes it might be better to have some longer lines if they improve
  readability or clarity.

## Readability

- Code explains itself. Names and structure carry the meaning, not comments.
- Concise constructs (ternaries, null coalescing, chaining, comprehensions and generator expressions) are welcome in
  moderation. Combining several in one statement is acceptable only while it stays obvious.
- Follow the patterns the codebase already uses.

## Correctness

Code does what it claims in every case it will meet, not only the one it was written for. Trace each path and check for:

- logic errors: wrong or inverted conditions, off-by-one boundaries, operator precedence
- inconsistencies: two places that should agree and do not, or a contract callers or callees no longer honour
- unhandled input: empty, missing, duplicate, very large, or malformed
- failures from I/O, the network, and other processes
- state and ordering: initialisation, cleanup, retries, partial failure, and concurrency where the code actually runs
  concurrently

Errors are never swallowed, misreported, or turned into a wrong result. Catching an error and returning a default is a
bug unless the default is correct.

Consider the consumer of errors: what will they do when the error is raised/thrown? Do the errors contain information
that the consumer cannot do anything with (internal details, ids, etc.)?

**Errors and validation have an outsize cost**
Despite their importance when dealing with untrusted input or unknown conditions,
Every added error or validation check has a triple cost:
 - a runtime overhead of doing the check, 
 - a much larger cognitive and general code maintenance cost
 - an ongoing double-implementation cost of keeping checks in sync with expectation
 
Before adding or broadening a validation check, or error flag/object, always consider what actually happens 
if that check is not there.  Does the logic naturally fail gracefully, or with an existing error?
What actual failure mode are you guarding against?  Does the cost outweigh the benefit? etc..
Is a generic KeyError/io::Error etc that the code already would raise enough for the consumer/use-case?

## Security

- Ask how each part could be subverted, misused, or abused, and what follows when it is.
- Watch for authentication bypass, privilege escalation, injection, sensitive data stored or sent unprotected, and
  services exposed across a boundary or left with default credentials.
- Question the approach, not only the line: the fix for a stored password may be not to use a password at all.
- Guard against concrete risks in the actual system, balanced against usability and complexity. Anything on the OWASP
  list is a real risk. Speculative hardening and security boilerplate are not.

## Tests

A bad test does more harm than a missing one. Aim for the right tests, not the most.

- Test behaviour, not implementation. A test that fails while the code still works is testing the wrong thing.
- Assert what matters and tolerate incidental detail: check that an error message contains the key information unless
  its exact text is a requirement.
- Cover ranges of input with parameterised or example-based tests, including edge cases.
- Avoid mocks. Prefer a real, isolated, fast instance of a dependency, so each unit runs in its true context.
- Each test reads on its own: its body shows what is being tested and what should happen. Balance shared helpers and
  fixtures against clarity; neither repeat setup endlessly nor hide intent behind infrastructure.
- Tests need no comments. The rare exception explains why a behaviour is expected or why a test exists.
- Keep the suite fast: create fixtures lazily, and share or cache expensive setup.
- Unit test code, including UI code, unless its risk is low. Add integration or end-to-end tests where the codebase
  warrants them, sparingly.
- Skipping tests is a cost-benefit judgement made honestly, never a way to avoid a hard test.

## Comments

- Do not comment what an expert can understand by skimming the code.
- Explain the non-obvious why: intent, constraints, trade-offs, rejected alternatives.
- Flag surprising behaviour, hidden assumptions, and hazards that could lead to incorrect changes or usage.
- Keep comments short, local, and accurate. Update or remove them with the code they describe.
- Do not repeat constants, numbers, or other code detail; the comment goes stale when the code changes.
- Just because a module may have a particular comment style, that does not override this guidance.

## Documentation

- Write concisely and directly. Fragments and bullet lists are fine when they are clearer.
- Capture semantics, summaries, and context the code cannot give. Leave out detail likely to change.
- Cut until nothing left can be removed without losing meaning.
- `DEVELOPMENT.md` is onboarding for the project as a whole: setup, dependencies, running the application, and running
  its tests and checks. Keep it current when those change. Never add notes about a particular change, story, or
  temporary workaround; they belong in the pull request.
