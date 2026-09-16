When considering architecture or code changes, look for code, functionality, and
modules that can be removed or refactored away.

Look for duplicated code, parallel or nearly parallel code paths, and legacy or
unused functions. Can a small tweak remove similar code, or an entire module,
without subverting the abstraction models?

If a migration path, legacy support, or backwards-compatibility function is
being added or retained, question whether it is needed at the project's current
stage, and avoid it wherever possible. A migration can often remove the need for
a legacy handler and its redundant code path.

Legacy or compatibility code that is genuinely required must be clearly marked
as deprecated, with the conditions under which it can be removed.
