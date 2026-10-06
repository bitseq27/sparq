# reference/observatory — the WO-020 INC4 convergence artefacts

Reviewed visuals for the host half of The Observatory. Regenerable, never hand-edited:

* `convergence-wo020-inc4.png` — the shell's convergence sheet at the reference wall breakpoint
  (2560×1600, Design chrome): the library grouped `INSTRUMENTS / The Observatory`, the toolbar's
  FOCUS door, and the wall-class card painting its AT-REST display list beside the backbone set
  (aurora heat band, coastline map, X-ray scatter, solar-wind cells, the ticker). The log line
  under it is the honest #58 refusal: PLAY with the wall on the canvas needs INC5's wasm loader.

Regenerate (sandbox or device):

```sh
# 1. the guest's at-rest render (also publishes it where the shell looks):
cargo run -p observatory-harness --manifest-path instruments-src/observatory/Cargo.toml \
  -- --out instruments-src/observatory/artefacts --atrest
# 2. the shell sheet (cairosvg for the PNG; the SVG itself is 5 MB of heat cells, kept out of git):
sparq ui --svg-out /tmp/convergence-wo020-inc4.svg --review --width 2560 --height 1600
python3 -c "import cairosvg; cairosvg.svg2png(url='/tmp/convergence-wo020-inc4.svg', write_to='reference/observatory/convergence-wo020-inc4.png', output_width=1700)"
# 3. the standalone wall at any LOD (the headless painter's door):
sparq instrument render --dir instruments/observatory --svg-out /tmp/wall-full.svg
```
