# kolorkei

A tiny [Bevy](https://bevy.org) toy that runs in the browser: press any key and a random
geometric shape in a vivid random color pops onto the screen with a random tone. On a phone,
tap anywhere instead: the shape appears under your finger. Shapes stay until you clear a random
half of them with **Space**, **Enter**, a **right click**, or a **two-finger tap**. The background
drifts toward the hue of the last shape.

## Controls

| Input                        | Effect                                  |
| ---------------------------- | --------------------------------------- |
| Any key                      | Random shape at a random spot           |
| Tap / left click             | Random shape under the finger or cursor |
| Space, Enter, right click    | Clear a random half of the shapes       |
| Two-finger tap               | Clear a random half of the shapes       |

**Play it:** https://paul-asvb.github.io/kolorkei/

## How it works

- Shapes are Bevy 2D mesh primitives (circle, rectangle, polygon, annulus, ellipse, rhombus,
  capsule, sector, segment, triangle), ring outlines of most of those, plus hand-built stars
  and crosses, each with a `ColorMaterial`. Many get one or two smaller shapes in contrasting
  hues nested inside. They pop in with a small overshoot and keep spinning slowly.
- Sounds are synthesized on the fly as 16-bit WAV data (sine / square / triangle / saw on a
  C-major pentatonic scale) and handed to Bevy's audio plugin, so no asset files are needed.
- Randomness is a small xorshift generator reseeded with the key and the press time, which
  avoids pulling `getrandom` into the WASM build.

## Development

Native:

```sh
cargo run
```

Browser (needs [trunk](https://trunkrs.dev) and the `wasm32-unknown-unknown` target):

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve --open
```

## Deployment

Every push to `main` runs `.github/workflows/deploy.yml`, which builds the release WASM bundle
with trunk and publishes it to GitHub Pages.
