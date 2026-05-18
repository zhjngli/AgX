# Getting started with AgX

Assumes AgX is installed — see [Install](../install.md).

In ten minutes, you'll edit your first photo two ways: by applying a preset (one command, full look) and by tweaking inline parameters (the slider model underneath). Both produce a real PNG on disk.

This tutorial uses a sample image and preset bundled in the AgX repository. If you cloned the repo, run the commands from its root. If you installed via `cargo install agx-cli` only, download the [`example/`](https://github.com/zhjngli/AgX/tree/main/example) directory or swap the paths for your own image and preset.

## Apply a preset

Run:

```bash
agx apply \
  -i example/images/cinque_terre_window.jpg \
  -p example/presets/golden-hour.toml \
  -o golden-hour.png
```

AgX decodes the source, renders it through every adjustment in the preset (tone, white balance, HSL, optional LUT — see the [preset model](../reference/concepts/preset-model.md)), and writes a new PNG.

![Original](../assets/tutorials/apply-before.jpg)
![After applying golden-hour.toml](../assets/tutorials/apply-after.jpg)

Open `golden-hour.png` in your image viewer. The result should be warmer, with lifted shadows and pulled-back highlights — a late-afternoon feel.

Try a different preset by swapping `-p`:

```bash
agx apply \
  -i example/images/cinque_terre_window.jpg \
  -p example/presets/moody-dark.toml \
  -o moody-dark.png
```

Each `.toml` file in `example/presets/` is a complete editing recipe. Presets are plain text — open one in your editor to see what's inside.

## Tweak the result with `edit`

A preset is just a saved bundle of parameters. To see the parameters themselves, use `edit` instead of `apply`:

```bash
agx edit \
  -i example/images/marina_sunset.heic \
  -o tweaked.png \
  --exposure 0.5 \
  --shadows 30 \
  --highlights -20
```

Three flags, three [basic adjustments](../explanation/algorithms/basic.md): brighten the image by half a stop, lift the shadows, pull back the highlights. The `agx edit` command exposes the same internals a preset addresses; the only difference is whether the values come from a `.toml` file or the command line.

![Original](../assets/tutorials/edit-before.jpg)
![After --exposure 0.5 --shadows 30 --highlights -20](../assets/tutorials/edit-after.jpg)

Try other flags. The full list lives in the [CLI reference](../reference/cli.md). Common ones:

- `--temperature` (warm/cool slider — positive warmer, negative cooler)
- `--contrast` and `--saturation`
- `--vignette-amount` (see the [vignette explanation](../explanation/algorithms/vignette.md))
- `--grain-amount` (see the [grain explanation](../explanation/algorithms/grain.md))

## What's next

You've seen the two foundational AgX commands. Where to go from here:

- [Apply a preset to a folder of photos](../how-to/batch-apply.md) — run `batch-apply` over a directory.
- [Compare looks side-by-side](../how-to/multi-apply.md) — `multi-apply` for preset audition.
- [Write your own preset](../how-to/write-preset.md) — author a TOML preset from scratch.
- [Compose layered looks](../how-to/compose-looks.md) — stack presets at apply time.
- [CLI reference](../reference/cli.md) — every subcommand and flag, generated from the source.
- [Preset format reference](../reference/preset.md) — every field, type, and default.
- [Algorithm explanations](../explanation/index.md) — how each adjustment works under the hood.
