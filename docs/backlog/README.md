# Backlog

Future work for AgX — epics, sub-tasks, and bugs. Each file is an "epic" with sub-tasks that can be worked on independently. When all sub-tasks in an epic are complete, remove the file from this directory.

Some epics have detailed sub-task docs (e.g., `grain-size-algorithm.md` is a sub-task of `processing-parity.md`). These live as their own files when the investigation is deep enough to warrant it, but are tracked under their parent epic in the roadmap.

## Modifying the backlog

When you spot something during other work (a new item, a completed sub-task, an obsoleted entry, a reframing), use the procedure below. The goal is for items to get filed, checked, moved, or removed in the right place when they surface — not later. After `/clear`, "what's in the backlog?" is answerable by scanning epic checkboxes; no recency index, no inbox, no hunting through git log.

### Where items live (sections within each epic)

| Section | Purpose | When used |
|---|---|---|
| `## Sub-tasks` | Active in-scope work, checkboxes in priority order | Always (or `## Sub-projects` for epics that sequence multiple design-doc-warranting work units, e.g. `color-management.md`) |
| `## Bug fixes` | Defects in shipped features | When the epic has known bugs |
| `## Parked` | Surfaced during other work or explicitly out of current scope | When the epic has any |
| `## Considerations` | Cross-cutting concerns, constraints, decisions | When the epic warrants it |
| `## Related` | Back-links to intersecting epics | When applicable |

The section heading **is** the type — no per-item tag prefix needed. Where used below, `## Sub-tasks` means either the standard heading or the `## Sub-projects` variant.

Epic-specific operational sections (e.g. `## Status` in `color-management.md`, `## How to Re-Profile` in `performance.md`) are fine when the epic genuinely needs them. They are not item containers and don't participate in the taxonomy; treat them as epic-level prose.

Section-as-type also disambiguates check-off vs delete on completion:

- `## Sub-tasks` is **arc** — checked items stay; they show what the epic accomplished.
- `## Bug fixes` is **negative space** — gone when fixed.
- `## Parked` is **future intent** — gone when picked up or dropped.

### Sizes

Three tiers, picked at add time based on **how much context future-readers will need to recognize the item**:

| Tier | Trigger | Shape |
|---|---|---|
| **fleeting** | Nameable in <10 words; recognizable from the name alone | One-line checkbox, no body |
| **standard** | Needs 1–3 sentences of context (where it surfaces, why it matters) | Checkbox + 1–3 sentence body — **describe the problem, not the solution** |
| **deep** | Investigation already exists (alternatives weighed, code refs, tradeoffs documented) | Own file under `docs/backlog/<name>.md`, linked from parent epic with a "Parent epic" blockquote |

`deep` is rare. Items earn their own file by being investigated — you do not promote an item to `deep` mid-PR.

### Describe the problem, not the solution

Mid-PR you're already in solution-headspace. Capturing a speculative fix calcifies it; months later the picker reads the solution suggestion and works toward it even if the right answer has shifted.

**Good (problem-framed):**

```markdown
- [ ] HSL adjustment loses wide-gamut headroom. Out-of-sRGB values get clipped at the RGB→HSL entry conversion, so this stage doesn't benefit from the wider working space the rest of the engine uses.
```

**Bad (solution-framed):**

```markdown
- [ ] Switch HSL to OKHsl polar OKLab. Convert via OKLab matrices and redo per-channel hue/saturation/luminance math.
```

The bad version starts with a technology decision, not the symptom. Capture the problem; let the solution be decided when the item is worked.

### Modification procedure

The procedure covers any backlog touch triggered by other work. Six operations:

| Operation | Trigger | Action |
|---|---|---|
| **Add** | New item surfaced | Run the Adding sub-procedure below |
| **Complete** | Existing item resolved by your work | Check `[x]` (Sub-tasks) or delete the line (Bug fixes / Parked) |
| **Remove** | Existing item obsolete | Delete the line — no "deprecated" marker; git diff captures it |
| **Rework** | Existing item's framing has shifted | Rewrite in place; **no notes** (see Reworking) |
| **Promote / demote** | Item changes status (Parked↔Sub-tasks, or epic graduation) | Move the line; for graduation, follow the Promoting steps |
| **Split** | One item partially completed, the rest is its own thing | Edit original to match what you did, check it off, add a new line for the rest |

The **scope test** (see knock-out heuristic below) applies to all operations. A modification that turns into significant work (30-min wordsmith, multi-file move) is scope creep; defer to deliberate triage.

#### Adding

**Step 0: search first.** Before writing a new item, grep or eyeball-scan the relevant epic(s) to confirm no existing item already covers this. Duplicates accumulate quickly otherwise.

If no duplicate exists:

1. **Scope test.** Does this expand the current PR's mental scope? See the knock-out heuristic below. If no, knock it out and mention in the commit.
2. **Pick section.**
   - Defect in shipped feature → `## Bug fixes`
   - New thing in the epic's active scope → `## Sub-tasks`
   - Follow-on from current work OR explicitly out of current scope → `## Parked`
   - Doesn't fit any epic → see step 4
3. **Pick size.** Nameable in 10 words → fleeting. Needs 1–3 sentences (problem only) → standard. Already investigated with refs and tradeoffs → deep file.
4. **Epic-sized?** Park it in `## Parked` of the most-relevant existing epic with an inline `(epic candidate)` marker. **Do not create a new epic file mid-PR.** Promotion happens when picked up — see Promoting / demoting.
5. **Cross-cutting?** If the item touches other epics:
   1. Add an inline link to the other epic on the item:

      ```markdown
      - [ ] Item description → [other-epic.md](other-epic.md)
      ```

   2. Add a back-link in the other epic's `## Related` section. ← easy to forget; this is where it lives.
6. **Commit** with `docs(backlog):` prefix.

#### Completing and removing items

Check or delete per the operations table above. Mention in the commit if the removal isn't self-explanatory — the PR captures the why.

#### Reworking

Item's framing has shifted. Rewrite the body in place; **do not add notes**.

Layered `Edit YYYY-MM-DD:` notes age worst — readers absorb the original premise first, then have to mentally subtract corrections. Two notes deep and the item is unparseable. Commit to one version of the framing; let git history hold the prior.

- **Minor correction** (typo, clearer phrasing): rewrite in place
- **Substantial reframing** (problem turned out different): rewrite; git captures prior
- **Item belongs in a different epic now**: delete and re-add in the correct epic. Don't keep a "moved" tombstone.

If you're not confident in the new framing, **don't touch it mid-PR**. Defer to deliberate triage.

#### Promoting / demoting

Graduation and demotion are deliberate triage steps, not mid-PR actions. The scope test applies — if it doesn't fit in the current PR's mental scope, defer.

- **Parked → Sub-tasks** (item becomes in active scope): move the line. No body rewrite needed.
- **Sub-tasks → Parked** (item leaves scope this round): move the line, add a 1-sentence reason in the body. If the line was already checked `[x]`, this is a reframing rather than a demotion — uncheck it and rewrite the body to match the new scope.
- **`(epic candidate)` → own epic file**: graduation.
  1. Create the new epic file with proper-epic shape (overview, sub-tasks, considerations, related)
  2. Use the parked item's body as the basis, rewriting to fit the epic format
  3. Add the new epic to the roadmap and by-category tables below
  4. Delete the line from the original epic's `## Parked`

#### Splitting

One item, partially completed, with the remainder naturally separate work:

- Edit the original to describe only what you did
- Check it off `[x]`
- Add a new line for the remaining work, sized and sectioned per the Adding procedure

If the parts are inseparable (same root cause), don't split — leave the checkbox unchecked and edit the body to reflect the partial state. A multi-part item isn't done until all parts are.

### Knock-out vs backlog heuristic

| Knock it out | Backlog it |
|---|---|
| Fix is in a file already being edited | Touches files outside the current PR |
| Purely mechanical (typo, dead import, obvious lint, missing bound check) | Requires a design decision |
| Tests already need regenerating anyway | Would force a separate test/golden regen pass |
| Single line or single function | Multi-file change |
| Don't need to read more code to understand it | Need to investigate before fixing |

If **all** signals point left, knock it out and mention in the commit message. If **any one** signal lands right, backlog it. The asymmetry is intentional — false positives ("I should have just done that") are cheap; false negatives ("this exploded the PR") are expensive.

### Removing epics

When all `## Sub-tasks` are done and no `## Bug fixes` or `## Parked` items remain, **delete the epic file** from `docs/backlog/`. The design doc in `docs/plans/` and the PR commits preserve everything that matters — the backlog epic was scaffolding for tracking, and tracking is done.

Edge cases:

- **Sub-tasks done, Parked items remain.** Move the parked items to the most-relevant other epic (with `(epic candidate)` markers if any feel epic-sized), then delete the file.
- **Substantially complete with optional remainder.** Leave the file but remove it from the priority tables below so it doesn't compete for attention. Delete once the optional work is done or dropped.
- **Individual item becomes stale.** Just delete the checkbox line. No "obsoleted" marker; git diff captures the removal.

All markdown links in backlog files are validated by `verify.sh`.

## Roadmap

Prioritized by alignment with the project philosophy: preset-first batch editing via CLI and API. Features that make presets more expressive or batch workflows faster come first. Developer velocity improvements are also high priority.

The [Documentation Initiative](documentation-initiative.md) is substantially complete — only optional theme/polish remains — so it no longer appears in the priority tables below.

### Near-term

Practical improvements to existing functionality — small scope, clear value.

| Priority | Idea | Why now |
|----------|------|---------|
| 1 | [Performance](performance.md) | P1-P5 parallelization + GPU shipped; remaining work is batch memory profiling, P6 SIMD, and a GPU CI runner |
| 2 | [HEIC/HEIF Support](heic-support.md) | iPhone is the most popular camera and ships HEIC by default; supporting it unblocks the largest single user-reach gap |
| 3 | [Processing Parity](processing-parity.md) | Per-feature algorithm verification against darktable/RawTherapee — long arc, ship one algo at a time |

### Mid-term

Larger efforts that expand AgX's capabilities or improve code quality.

| Priority | Idea | Why next |
|----------|------|----------|
| 4 | [Color Management](color-management.md) | Wide gamut and ICC profiles close the consumer/professional gap and unlock log + wide-gamut working spaces other features depend on |
| 5 | [Preset Tooling](preset-tooling.md) | `agx validate` and apply-time unknown-field warnings shipped; schema versioning + variables become urgent on first breaking schema change |
| 6 | [Pluggable Pipeline](pluggable-pipeline.md) | Stage trait + per-pixel/neighborhood extraction shipped; remaining sub-tasks (stage caching, color-space auto-insert) gate on interactive UI work and wider gamut support |
| 7 | [Ecosystem Interop](ecosystem-interop.md) | XMP/costyle/pp3 import — parameter mapping is inherently approximate; defer until concrete user demand appears |

### Long-term

Major features that require significant design work or change the project's scope.

| Priority | Idea | Notes |
|----------|------|-------|
| 9 | [Geometric Corrections](geometric-corrections.md) | Lens corrections, perspective, crop/rotation |
| 10 | [Local Adjustments](local-adjustments.md) | Major architectural change to the render model |
| 11 | [Platform and Distribution](platform-and-distribution.md) | REST API, GPU, WASM, preset marketplace |
| 12 | [UI](ui.md) | Desktop and web interfaces |
| 13 | [Advanced Research](advanced-research.md) | AI editing, HDR merge, panorama, focus stacking |

## By Category

### Editing

| File | Summary |
|------|---------|
| [local-adjustments.md](local-adjustments.md) | Brushes, gradients, and radial filters for per-region edits |
| [geometric-corrections.md](geometric-corrections.md) | Lens corrections, perspective, crop and rotation |

### Pipeline and Infrastructure

| File | Summary |
|------|---------|
| [performance.md](performance.md) | Data-driven render optimization roadmap (P1-P5 + P7 GPU shipped; memory and SIMD remaining) |
| [pluggable-pipeline.md](pluggable-pipeline.md) | Stage-based render pipeline with caching and color-space awareness |
| [preset-tooling.md](preset-tooling.md) | Schema versioning, validation, and authoring shortcuts |

### Quality and Correctness

| File | Summary |
|------|---------|
| [processing-parity.md](processing-parity.md) | Per-feature algorithm verification, grain size bug fix, raw processing improvements |

### Documentation

| File | Summary |
|------|---------|
| [documentation-initiative.md](documentation-initiative.md) | Umbrella epic: mdbook site, rustdoc retrofit, auto-gen CLI/preset ref, algorithm explanations, tutorials, how-tos |

### Color and Formats

| File | Summary |
|------|---------|
| [color-management.md](color-management.md) | Wide gamut, ICC profiles, per-camera color matrices |
| [heic-support.md](heic-support.md) | HEIC/HEIF format decoding for Apple devices |
| [ecosystem-interop.md](ecosystem-interop.md) | XMP/costyle/pp3 import/export and sidecar files |

### Platform and UI

| File | Summary |
|------|---------|
| [platform-and-distribution.md](platform-and-distribution.md) | REST API, GPU, WASM, preset marketplace |
| [ui.md](ui.md) | Desktop and web UI, histogram, before/after, undo/redo |
| [advanced-research.md](advanced-research.md) | AI editing, HDR merge, panorama, focus stacking |
