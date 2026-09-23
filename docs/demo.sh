#!/bin/sh
# Regenerates docs/demo.gif. Needs vhs and ffmpeg.
# VHS's built-in GIF encoding fails with ffmpeg 9, so VHS only renders
# frames and ffmpeg assembles them here.
set -eu
cd "$(dirname "$0")/.."

cargo build --release -q
rm -rf target/demo-frames
vhs docs/demo.tape

# Cursor frames are drawn over text frames, padded with the theme background;
# one palette for the whole clip.
ffmpeg -y -loglevel error \
  -framerate 50 -i target/demo-frames/frame-text-%05d.png \
  -framerate 50 -i target/demo-frames/frame-cursor-%05d.png \
  -filter_complex "[0][1]overlay,pad=iw+48:ih+48:24:24:color=0x1e1e2e,fps=20,split[a][b];[a]palettegen=max_colors=256:stats_mode=full[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
  -loop 0 docs/demo.gif
echo "wrote docs/demo.gif"
