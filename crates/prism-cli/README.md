# rsx-prism

**P**robably **R**epairs **I**nconsistent **S**e**m**i-transparency.

Most image editors write black into the RGB channels of fully transparent pixels. You cannot see it — alpha is zero — but GPUs do not sample colour and alpha independently. Bilinear filtering and mipmap generation blend that hidden black straight back in, and a sprite that looked clean in Photoshop picks up a dark halo in-engine.

prism replaces that hidden RGB with the colour of the nearest *visible* pixel. **Alpha is never touched.**

```
cargo install rsx-prism
```

> The crate is published as `rsx-prism` because `prism` was taken. The installed command is `prism`.

## Usage

```
prism sprite.png                 # writes sprite.fixed.png
prism art/ -r                    # whole folder, recursively
prism art/ -i                    # overwrite in place
prism art/ -o build/             # write results somewhere else
prism sprite.png --dry-run       # tell me what you'd do, don't do it
```

### The two alpha knobs

prism splits the alpha range into three bands:

| alpha | what happens |
| --- | --- |
| `>= --seed-alpha` (default 255) | trusted colour source, never modified |
| `<= --fill-alpha` (default 0) | RGB rewritten from the nearest source |
| anything between | left completely alone |

Defaulting `--seed-alpha` to 255 matters more than it looks: a pixel at `alpha = 1` with its RGB already crushed to black is exactly the garbage being removed, so it must not be allowed to seed the fill.

If your source has soft edges that are *also* tinted, raise `--fill-alpha` (try `128`) to bring those into scope.

## Formats

`.png` and `.webp` work out of the box. AVIF is behind a feature flag because decoding it links against the system `dav1d` library:

```
cargo install rsx-prism --features avif
```

## Licence

MIT
