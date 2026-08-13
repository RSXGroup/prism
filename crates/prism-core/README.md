# rsx-prism-core

Repairs the RGB data hidden underneath transparent pixels.

Most image editors write black into the RGB channels of fully transparent pixels. It is invisible — alpha is zero — but GPUs do not sample colour and alpha independently, so bilinear filtering and mipmap generation blend that hidden black back in and the sprite picks up a dark halo.

This crate rewrites that hidden RGB with the colour of the nearest visible pixel. **Alpha is never modified.**

```rust
use prism_core::{FillOptions, fill_rgba};

// 2×1 RGBA: an opaque red pixel and a transparent black one.
let mut pixels = vec![255, 0, 0, 255, /**/ 0, 0, 0, 0];
let stats = fill_rgba(&mut pixels, 2, 1, FillOptions::default()).unwrap();

assert_eq!(stats.filled, 1);
assert_eq!(&pixels[4..8], &[255, 0, 0, 0]); // red now, still invisible
```

> The crate is published as `rsx-prism-core` because `prism-core` was taken. The importable name is `prism_core`.

## How

An exact Euclidean feature transform ([Felzenszwalb & Huttenlocher](https://cs.brown.edu/people/pfelzens/papers/dt-final.pdf)): two linear passes, `O(width × height)` regardless of how many opaque pixels there are, and mathematically exact rather than approximate like jump flooding.

Enable the `parallel` feature to spread the row pass across cores with rayon. Leave it off for wasm.

## Scope

No codecs, no file I/O, no dependencies by default. You bring a decoded RGBA8 buffer. That is what lets the same code back both the [`rsx-prism`](https://crates.io/crates/rsx-prism) CLI and the browser build.

**If you are decoding in a browser, do not use `<canvas>`.** Canvas stores premultiplied alpha, so the `getImageData` round trip destroys the RGB precision of exactly the semi-transparent pixels this crate exists to repair.

## Licence

MIT
