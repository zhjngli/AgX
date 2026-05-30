# Asset licensing policy

AgX is published as a Rust library (`agx-photo` on crates.io) under MIT OR Apache-2.0. Any binary asset shipped with AgX — ICC color profiles, `.cube` LUTs, sample images, font files, anything that is "data, not code" — must be license-compatible with that posture.

## Allowed

- **Public domain / CC0.** No obligations, no propagation.
- **MIT / BSD / Apache-2.0.** Standard permissive. Requires attribution somewhere in the distribution; trivial to satisfy.
- **Self-generated.** Anything AgX produces via its own dev tools (LUTs from `agx-lut-gen`, ICC profiles from `agx-profile-gen`) is treated as MIT/Apache under the AgX license. The generator tool itself must also be permissively licensed — lcms2 (MIT) is the established choice for ICC.

## Rejected — and why

- **Share-alike (CC-BY-SA, GPL applied to assets).** The share-alike clause arguably propagates to AgX's output files: an edited photo containing CC-BY-SA bytes could be read as a derivative work of the share-alike asset, which would attach a share-alike obligation to the user's edited photo. Even under the looser reading where photo files escape that clause, share-alike asset bytes inside an MIT/Apache *library* contaminate the library's license posture — downstream library consumers cannot rely on MIT/Apache semantics for their entire dependency tree. GIMP and darktable embed CC-BY-SA profiles freely because they are GPL projects; AgX cannot.
- **"Free to use" without a named license.** The original sRGB ICC reference profile from HP / Microsoft / the ICC Consortium falls here. Vague terms are not auditable; downstream legal review cannot clear them. Skip.
- **Vendor reference profiles** (Adobe RGB from Adobe, Display P3 from Apple). Actively copyrighted, no general license grant. Skip.

## Past decisions

| Asset | Considered | Rejected because | Replaced with |
|---|---|---|---|
| sRGB v4 ICC | Elle Stone `sRGB-elle-V4-srgbtrc.icc` | CC-BY-SA 3.0 (verified in profile cprt tag and repo LICENSE file). Share-alike. | lcms2-generated profile via `crates/agx-profile-gen`. lcms2 is MIT. |

## How to add a new asset

1. Locate a license-compatible source (see "Allowed" above) or generate the asset from primary data using a permissive-licensed tool (lcms2, our own code).
2. Document the source in this table.
3. Add the asset bytes to its module's `profiles/` (or equivalent) directory.
4. Add a one-line attribution comment in the consuming Rust file pointing at this policy doc.

For ICC profiles specifically, the generator pattern is the right default: it gives full provenance and avoids any third-party license question. See [`crates/agx/src/encode/profiles/README.md`](../../crates/agx/src/encode/profiles/README.md) for the in-tree example.
