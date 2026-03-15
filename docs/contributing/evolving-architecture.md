# Evolving the Architecture

This document describes when and how to change the module layering rules enforced by `crates/agx/tests/architecture.rs`. Read `ARCHITECTURE.md` first for the current rules.

## When to Change the Architecture

Change the layering when:

- **A new feature requires a forbidden dependency.** You need module A to import from module B, but the current rules prohibit it, and there is no reasonable way to restructure the code to avoid the dependency.
- **A module's responsibility has outgrown its current boundaries.** The module is doing work that belongs elsewhere, or a new module is needed to hold a concept that does not fit any existing module.

## When NOT to Change the Architecture

Do not change the layering when:

- **You can restructure to avoid the dependency.** Most structural test failures are accidental. Common fixes:
  - Move a type to a lower-level module (e.g., move a shared struct to `error` or a new shared module).
  - Pass a value as a function parameter instead of importing the type directly.
  - Re-export a type from the correct module boundary.
  - Extract an interface (trait) that lives at a lower level.
- **The dependency is only needed in tests.** Test code can import freely; the structural test only scans non-test source files. If you only need the cross-module import in a `#[cfg(test)]` block, no rule change is required.
- **The change would only benefit a single call site.** If one function in one file needs the import, that is a sign the function may belong in a different module rather than that the boundary should move.

## Process

When a genuine architecture change is needed, follow these steps in order:

1. **Identify the conflict.** State which module needs to import from which other module, and why. Be specific: name the types or functions involved.

2. **Write it in a design doc.** Add a section to the relevant feature design doc in `docs/plans/` explaining:
   - What dependency is needed and why.
   - Why restructuring cannot avoid it.
   - What the new dependency graph looks like.
   - What negative constraints change (if any).

3. **Get approval.** Architecture changes affect the entire codebase. The change must be reviewed and approved before implementation. For agent-driven work, surface the conflict to the human and wait for a decision.

4. **Update `ARCHITECTURE.md`.** Change all affected sections:
   - Module dependency graph (ASCII diagram).
   - Dependency rules table.
   - Negative constraints section.
   - Core invariants (if affected).

5. **Update affected module READMEs.** Each module's `README.md` documents its public API, dependencies, and negative constraints. Update any that are affected by the boundary change.

6. **Update the structural test.** Modify `crates/agx/tests/architecture.rs` to reflect the new rules. Add or remove forbidden imports as needed. Update the test function name and doc comment to match.

7. **Implement the feature.** With the rules updated, write the code that requires the new dependency.

8. **Verify.** Run `cargo test` and confirm that all structural tests pass with the new rules, and that no unintended cross-module imports were introduced.

## Principles

- **Prefer constraining over expanding.** Adding a new rule ("module X must not import from Y") is preferred over removing one. If you must remove a rule, clearly document why the constraint is no longer valid.
- **Document coupling explicitly.** When a new dependency is added between modules, the "May import from" column in the dependency rules table should state exactly which types or functions are allowed, not just the module name. This prevents a narrow exception from becoming a broad coupling.
- **Make small changes.** Change one boundary at a time. If a feature requires multiple layering changes, break it into steps and verify after each one.

## Agent Guidance

When an AI agent encounters a structural test failure:

1. **Read the assertion message.** It names the module, the forbidden import, and the exact file and line.

2. **Read `ARCHITECTURE.md`** to understand why the rule exists and what the module boundaries are.

3. **Try restructuring first.** This is the common case. Move a type, extract a parameter, re-export from a different module. Most violations are accidental and do not require a rule change.

4. **If restructuring is not possible**, do not suppress or work around the test. Instead:
   - Note in the current design doc (or create a short one) what dependency is needed and why.
   - Surface the conflict to the human for decision.
   - Wait for approval before changing any rules in `ARCHITECTURE.md` or `architecture.rs`.

The goal is to keep boundary changes visible and intentional. The structural test exists so that accidental coupling is caught early. Changing the rules is fine when it is the right thing to do -- the process just ensures it happens deliberately.
