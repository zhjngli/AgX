# Dehaze

## Pipeline

```mermaid
flowchart TD
    I["Input I (linear RGB)"] --> DC["dark_channel(I) -- 15x15 patch min"]
    DC --> A["estimate_airlight -- top 0.1% of dark channel"]
    A --> Branch{"amount sign"}
    Branch -- negative --> Blend["I*(1 - s) + A*s, s = -amount/100"]
    Branch -- positive --> Norm["normalize: I / max(A, 0.01)"]
    Norm --> DCN["dark_channel(I/A)"]
    DCN --> Traw["t_raw = 1 - omega * dc_norm, omega = amount/100"]
    I --> Guide["luma guide (Rec. 709)"]
    Guide --> GF["guided_filter -- radius 40, eps 0.001"]
    Traw --> GF
    GF --> Tref["t (refined)"]
    A --> Rec
    Tref --> Rec["recover: J = (I - A) / max(t, 0.1) + A"]
    Rec --> Out["Output J, clamp 0..1"]
    Blend --> Out
```

Positive `amount` runs the full Dark Channel Prior recovery path; negative `amount` reuses the airlight estimate to add scene-aware fog and skips the transmission and guided-filter stages.

{{#include ../../../../crates/agx/src/adjust/dehaze.md}}

## See also

- Concept references: [Detail](../reference/concepts/detail.md) (dehaze entry)
- API references: [dehaze](../api/agx/adjust/dehaze/index.html)
- Related explanations: [Detail pass](detail.md), [Noise reduction](denoise.md)
