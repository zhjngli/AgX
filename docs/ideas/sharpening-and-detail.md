# Sharpening and Detail

**Category:** Editing
**Status:** Backlog

## Problem / Opportunity

Sharpening, clarity, texture, and noise reduction are essential detail-level adjustments. Sharpening recovers detail lost in demosaicing and lens softness. Clarity and texture enhance local contrast at different frequency scales. Noise reduction removes sensor noise while preserving detail. Together they control the perceived detail and quality of the final image.

## Key Considerations

- **Sharpening**: Unsharp mask (amount, radius, detail, masking) is the standard approach. Masking uses luminance edges to avoid sharpening noise in smooth areas
- **Clarity**: Local contrast enhancement at medium frequencies — emphasizes textures and edges without halos. Typically implemented via a local tone mapping or frequency separation approach
- **Texture**: Similar to clarity but targets higher frequencies — fine detail without large-scale contrast changes
- **Noise reduction**: Separate luminance NR and color NR with detail preservation slider. Wavelet-based or bilateral filtering approaches
- All of these are neighborhood operations (require access to surrounding pixels), unlike current per-pixel adjustments — this has pipeline architecture implications
- Must operate on the full image buffer, not pixel-by-pixel — may benefit from the pluggable pipeline stage design
- Sharpening should happen late in the pipeline (after tone/color adjustments, before output encoding)

## Related

- [Pluggable Pipeline](pluggable-pipeline.md) — neighborhood operations need buffer access, motivating the stage-based architecture
- [Film and Grain](film-and-grain.md) — grain is applied after sharpening/NR
