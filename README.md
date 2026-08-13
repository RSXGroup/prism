# prism

> **P**robably **R**epairs **I**nconsistent **S**e**m**i-transparency
>
> a spiritual fork of [`Transparent-Pixel-Fix` by Corecii](https://github.com/Corecii/Transparent-Pixel-Fix), is it any better? you tell me
>
> mostly tailored for roblox creators, but it'll happily fix any image

---

## the problem

most image editors write **black** into the RGB channels of fully transparent pixels. you can't see it, alpha is zero, but GPUs don't sample colour and alpha independently. bilinear filtering and mipmap generation blend that hidden black straight back in, and the sprite that looked clean in photoshop picks up a dark halo in-engine.

prism replaces that hidden RGB with the colour of the nearest *visible* pixel. **alpha is never touched.** the image looks byte-for-byte identical to a human; it just samples correctly now.

---

## using it

```
prism sprite.png                 # writes sprite.fixed.png
prism art/ -r                    # whole folder, recursively
prism art/ -i                    # overwrite in place
prism art/ -o build/             # write results somewhere else
prism sprite.png --dry-run       # tell me what you'd do, don't do it
```

```
Usage: prism [OPTIONS] [PATH]...

Arguments:
  [PATH]...              Images or folders to process

Options:
  -r, --recursive        Descend into subfolders when a path is a directory
  -s, --suffix <TEXT>    Text inserted before the extension [default: fixed]
  -o, --out-dir <DIR>    Write results into this directory instead
  -i, --in-place         Overwrite the source files
  -j, --jobs <N>         Files to process at once [default: CPU cores]
      --seed-alpha <N>   Min alpha to be trusted as a colour source [default: 255]
      --fill-alpha <N>   Max alpha to have its colour rewritten [default: 0]
      --dry-run          Report what would change without writing anything
  -q, --quiet            Only print errors and the final summary
  -h, --help             Print help
  -V, --version          Print version
```

### the two alpha knobs

these are the interesting ones. prism splits the alpha range into three bands:

| alpha | what happens |
| --- | --- |
| `>= --seed-alpha` | trusted colour source, never modified |
| `<= --fill-alpha` | RGB rewritten from the nearest source |
| anything between | left completely alone |

the defaults (`255` / `0`) mean **only fully opaque pixels get to seed the fill**, which matters more than it sounds. a pixel sitting at `alpha = 1` with its RGB already crushed to black is exactly the garbage we're trying to delete — letting it seed would just smear the problem outwards.

if your source has soft edges that are *also* tinted, raise `--fill-alpha` (try `128`) to bring those into scope too.

---

## building it

needs rust 1.85+.

```
cargo build --release
```

binary lands at `target/release/prism`. run the tests with `cargo test --workspace`.

`.png` and `.webp` work out of the box. AVIF is behind a feature flag because decoding it links against the system `dav1d` library:

```
cargo build --release --features avif
```

---

## what's inside

```
crates/
  prism-core   the algorithm. no codecs, no file I/O, no deps.
  prism-cli    the binary you actually run.
  prism-wasm   thin wasm-bindgen wrapper for the browser build.
```

`prism-core` takes a decoded RGBA8 buffer and nothing else, which is what lets the exact same code run in the CLI and in a browser tab.

the fill itself is an **exact euclidean feature transform** ([Felzenszwalb & Huttenlocher](https://cs.brown.edu/people/pfelzens/papers/dt-final.pdf)) — two linear passes, O(width × height) no matter how many opaque pixels there are, and mathematically exact rather than approximate like jump flooding. a 4096×4096 image with 11.2M transparent pixels goes through the whole decode → repair → re-encode round trip in about a second.

---

## a note on the web version

it's coming, and it'll run entirely in your browser — no upload, no server.

if you're building against `prism-wasm` yourself: **don't decode images with `<canvas>`.** canvas stores premultiplied alpha, so the `getImageData` round trip destroys the RGB precision of exactly the semi-transparent pixels prism exists to repair. use a real decoder like [`@jsquash/png`](https://github.com/jamsinclair/jSquash) and pass the raw buffer straight in.

---

> im sure there are many ways to improve it, so do open an issue or hmu on [discord](http://discord.com/users/1160943839591288893)
> if you do have any ideas
>
> thaank youuuu x
