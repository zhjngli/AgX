# Backlog Taxonomy and Add-Time Procedure

## Problem

`docs/backlog/` works at the file-and-checkbox level, but the **process for adding items during other work** is unspecified. Items discovered mid-PR get dropped into the most-relevant epic's prose section ("Known gaps", "Deferred / out of scope", or an ad-hoc paragraph) with no consistent shape. Across the 14 epic files today, the same idea is captured in three different conventions, and there's no shared heuristic for:

- **When to knock something out** vs. add it to the backlog.
- **Where it lives** within an epic (sub-task vs. follow-up vs. deferred vs. bug).
- **How much detail** to capture (one-line note vs. paragraph vs. own file).
- **How to handle cross-cutting items** that touch multiple epics.

Symptoms:

- After `/clear`, finding "what was added recently" requires reading every epic cover-to-cover or scanning git log by hand. The orientation-double-application bug filed during PR #52 only got picked up two days later because it stayed top-of-mind, not because the backlog surfaced it.
- Color management SP1 (PR #51) shipped with five cross-cutting "Deferred" items in `color-management.md`, each a ~200-word prose paragraph. Reading them is heavy; picking which is the cheapest next thing requires absorbing all five.
- Items added mid-PR sometimes commit to a fix shape ("switch HSL to OKHsl") that may not survive contact with the actual work. The framing pulls future implementation toward a speculative solution.

The current `docs/backlog/README.md` has format guidance ("Adding New Items") but it's buried in prose, doesn't cover follow-ups or cross-cutting items, and gives no heuristic for the at-add-time decisions.

## Goals

- Normalize section headings inside each epic so item type is encoded in **where it lives**, not in a tag prefix.
- Spell out an **add-time procedure** that fits on the back of a postcard: scope test → pick section → pick size → write → commit.
- Define a **knock-out-vs-backlog heuristic** so mid-PR decisions don't stall on time-estimation.
- Define **size tiers** with the discipline that standard items capture the problem, not a speculative fix.
- Define **cross-cutting handling** that uses single-home + opportunistic back-link, no duplication.
- Migrate the existing 14 backlog files to the new conventions in the same PR. The convention coexisting with old prose blocks would weaken adoption.
- Update `CLAUDE.md` to point at the new procedure so future AI-assisted sessions follow it consistently.

## Non-goals

- Per-item frontmatter, tags, or YAML metadata. The section heading is the type.
- A CI validator or lint that enforces backlog hygiene. Convention is enforced by review, not tooling.
- An `_inbox.md` or "recently added" index file. Items get triaged at add time, not parked.
- Automated cross-reference detection or back-link generation.
- Restructuring the existing epic file boundaries. Same files, same purposes — only section headings and item-level conventions change.
- Restructuring the README roadmap or by-category tables. Those work as-is.

## Design

### Section taxonomy (per epic)

Standardize on five sections within each epic file:

| Section | Purpose | When used |
|---|---|---|
| `## Sub-tasks` | Active in-scope work, checkboxes in priority order | Always (or `## Sub-projects` for epics that sequence multiple design-doc-warranting work units, e.g. `color-management.md`) |
| `## Bug fixes` | Defects in shipped features | When the epic has known bugs |
| `## Parked` | Surfaced during other work or explicitly out of current scope | When the epic has any |
| `## Considerations` | Cross-cutting concerns, constraints, decisions | When the epic warrants it (existing pattern) |
| `## Related` | Back-links to intersecting epics | When applicable |

`## Parked` collapses the previous "Followups" / "Deferred" / "Known gaps" / "Deferred / out of scope" headings into a single bucket. The provenance distinction (discovered-during-work vs. planned-out-of-scope) was real but not actionable — both buckets meant "we may or may not get to this, not active scope right now."

### Cross-cutting items

A single epic owns the item — typically the epic where the work will get done, not where the item surfaced. The item carries an inline link to other relevant epics:

```markdown
- [ ] Gamut compression on encode — affects [color-management.md](color-management.md) and [processing-parity.md](processing-parity.md). Hard-clipping at sRGB output produces flat-mesa artifacts on vivid sunsets.
```

The *other* epics get a back-link in their `## Related` section. No duplication of the item itself. If the item is genuinely owned by multiple epics, pick the one whose name most strongly implies "this is where this gets worked" and accept the choice.

### Size tiers

Three tiers, picked at add time based on **how much context future-readers will need to recognize the item**:

| Tier | Trigger | Shape |
|---|---|---|
| **fleeting** | Nameable in <10 words; you'll recognize it later from the name alone | One-line checkbox, no body |
| **standard** | Needs 1–3 sentences of context (where it surfaces, why it matters) | Checkbox + 1–3 sentence body; **describes the problem, not the solution** |
| **deep** | Investigation already exists (alternatives weighed, code refs, tradeoffs documented) | Own file under `docs/backlog/<name>.md`, linked from parent epic |

`deep` is rare on purpose. Items earn their own file by being investigated — you do not promote an item to `deep` mid-PR. Examples that earned `deep` today: `grain-size-algorithm.md` (full bug investigation with proposed fixes and code analysis).

### "Standard items describe the problem, not the solution"

This is the most-likely-to-drift rule. Mid-PR you're already in solution-headspace. Mitigate by example.

**Good (problem-framed):**

```markdown
- [ ] HSL adjustment loses wide-gamut headroom. Out-of-sRGB values get clipped at the RGB→HSL entry conversion, so this stage doesn't benefit from the wider working space the rest of the engine uses.
```

**Bad (solution-framed):**

```markdown
- [ ] Switch HSL to OKHsl polar OKLab. Convert via OKLab matrices and redo per-channel hue/saturation/luminance math.
```

The bad version commits to a fix mid-PR. When picked up six months later, OKHsl might not be the right call (a simpler decompose-and-recombine approach might suit, or the requirement might have shifted), but the framing pulls implementation toward it anyway. Capture the problem; let the solution be decided when the item is worked.

### Add-time procedure

This is the back-of-postcard checklist that goes in `docs/backlog/README.md`:

1. **Scope test.** Does this expand the current PR's mental scope? See knock-out heuristic below. If no, knock it out, mention in commit.
2. **Pick section.** Which heading in which epic does it belong under?
   - Defect in shipped feature → `## Bug fixes`
   - New thing in the epic's active scope → `## Sub-tasks`
   - Follow-on from current work OR explicitly out of current scope → `## Parked`
   - Doesn't fit any epic → see step 4
3. **Pick size.** Nameable in 10 words → fleeting. Need 1–3 sentences (problem only) → standard. Already investigated with refs and tradeoffs → deep file.
4. **Epic-sized?** If the item feels epic-sized when adding (own file, own roadmap entry, multiple sub-tasks), park it in `## Parked` of the most-relevant existing epic with an inline `(epic candidate)` marker. **Do not create a new epic file mid-PR.** Promotion to its own file happens when someone deliberately picks it up.
5. **Cross-cutting?** If the item touches other epics:
   1. Add an inline `→ [other-epic.md](other-epic.md)` link on the item
   2. Add a back-link in the other epic's `## Related` section ← easy to forget; this is where it lives
6. **Commit.** Conventional commit prefix `docs(backlog):` so it's findable via git log.

### Knock-out heuristic

Time estimates are unreliable. The real test is scope creep:

| Knock it out | Backlog it |
|---|---|
| Fix is in a file already being edited | Touches files outside the current PR |
| Purely mechanical (typo, dead import, obvious lint, missing bound check) | Requires a design decision |
| Tests already need regenerating anyway | Would force a separate test/golden regen pass |
| Single line or single function | Multi-file change |
| Don't need to read more code to understand it | Need to investigate before fixing |

If **all** signals point to the left column, knock it out and mention in the commit message. If **any one** signal lands in the right column, backlog it. The asymmetry is intentional — false positives ("I should have just done that") are cheap; false negatives ("this exploded the PR") are expensive.

## Documentation Updates

This change touches three surfaces; all three must update in the implementation PR.

- **`docs/backlog/README.md`** — replace the current "Adding New Items" prose with:
  - Section taxonomy table (5 standard headings)
  - Size tier table with examples
  - The numbered add-time procedure (6 steps)
  - The knock-out heuristic table
  - "Good vs bad framing" example for the problem-not-solution rule
  - Cross-cutting handling block (inline link + back-link instructions)
- **`docs/backlog/*.md`** (14 files) — one-shot migration:
  - Rename `## Known gaps`, `## Deferred / out of scope for this epic`, `## Deferred`, `## Followups (post-MVP follow-up)` → `## Parked`
  - Keep `## Sub-tasks`, `## Bug fixes`, `## Considerations`, `## Related` as-is where present
  - Add missing `## Related` sections if a known cross-cutting reference exists and the section is absent
  - Audit existing items for solution-framing creep; leave alone if not blatant (don't rewrite history)
- **`CLAUDE.md`** — replace the current `## Backlog` section to point at the new README procedure. Keep the type-declaration requirement, drop the inline three-level bullets (now in README).

## Migration plan

One-shot pass during the implementation PR. Inventory across the 14 epic files shows the migration is small — only two heading renames needed; most epics already follow the standard shape.

| File | Change |
|---|---|
| `heic-support.md` | Rename `## Known gaps (post-MVP follow-up)` → `## Parked` |
| `color-management.md` | Rename `## Deferred / out of scope for this epic` → `## Parked` |
| `color-management.md` | Keep `## Sub-projects` as-is (intentional variant for sequenced multi-design-doc epics; `## Sub-tasks` is the standard but `## Sub-projects` is accepted for this shape) |
| `grain-size-algorithm.md` | Leave as-is — this is a `deep` tier sub-task file with its own investigation shape (`## Problem`, etc.), not an epic |
| `performance.md` | Missing `## Related` section — add one if cross-cutting refs exist; otherwise leave |
| All others (10 files) | Already match the standard shape — no rename needed |

Existing items are **not retroactively reorganized** beyond the heading rename. The discipline of "problem not solution" applies to new items only; existing items stay as-is unless they're obviously broken.

The README rewrite acknowledges `## Sub-projects` as an accepted variant for epics that sequence multiple design-doc-warranting work units.

## Holes and accepted tradeoffs

Honest list of what this design does **not** solve, with reasoning for accepting the cost:

1. **Problem-not-solution drift cannot be enforced.** The README example raises the floor but doesn't catch slips. Accept: the cost of catching it (a CI lint on item bodies) is higher than the value.
2. **Cross-cutting back-link maintenance is manual.** When adding an item in epic A linked to epic B, also updating epic B's `## Related` is a discipline question. Accept: making the step explicit in the procedure is the best mitigation short of automation.
3. **No staleness check.** Items written months ago may no longer match current code or thinking. Accept: epic-pickup is the implicit triage moment. The picker re-reads and prunes before working.
4. **`(epic candidate)` markers have no trigger.** They sit in `## Parked` until someone notices. Accept: the marker is metadata, not a queue. Promotion is deliberate, not scheduled.
5. **README becomes load-bearing.** The procedure, heuristics, and section conventions all live in one file. If the README rots, the convention rots. Accept: the README is in active use anyway; rot risk is small.
6. **Design relies on convention + judgment, no automation.** Accept: this is right-sized for the project. If discipline slips, that's the signal to add frontmatter and a validator — but until then, simpler wins.

## Out of scope (explicit non-decisions)

- **No memory integration.** auto-memory could surface "recent backlog additions" but this design deliberately doesn't depend on it.
- **No frontmatter schema** for backlog items. Plain markdown checkboxes only.
- **No automation** for cross-cutting detection or backlink validation.
- **No reorganization of file boundaries.** Same 14 files. Renaming or merging epics is a separate decision.
- **No retroactive rewrite** of existing item bodies for solution-framing. New items follow the rule; existing items stay as-is.

## Related

- [`docs/backlog/README.md`](../backlog/README.md) — the surface this design rewrites
- [`CLAUDE.md`](../../CLAUDE.md) — `## Backlog` section gets updated to point at the new procedure
