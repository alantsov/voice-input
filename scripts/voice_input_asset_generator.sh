#!/usr/bin/env bash
set -euo pipefail

# Resolve script and repo roots
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Configuration
SRC_SVG="$REPO_ROOT/assets/icon-src/voice-input.svg"
OUT_ROOT="$REPO_ROOT/assets/icons/hicolor"
SIZES=(16 22 24 32 48)

# Define your state colors (hex). Adjust to your palette if needed.
declare -A COLORS=(
  [blue]="#3B82F6"
  [red]="#EF4444"
  [yellow]="#F59E0B"
  [white]="#FFFFFF"
)

# Tools: prefer rsvg-convert; fallback to Inkscape
have_rsvg=false
have_inkscape=false
if command -v rsvg-convert >/dev/null 2>&1; then
  have_rsvg=true
elif command -v inkscape >/dev/null 2>&1; then
  have_inkscape=true
else
  echo "Error: Need rsvg-convert (librsvg) or inkscape installed." >&2
  exit 1
fi

# Ensure input exists
if [[ ! -f "$SRC_SVG" ]]; then
  echo "Error: Source SVG not found: $SRC_SVG" >&2
  exit 1
fi

# Prepare output directories
mkdir -p "${OUT_ROOT}/scalable/apps"
for sz in "${SIZES[@]}"; do
  mkdir -p "${OUT_ROOT}/${sz}x${sz}/apps"
done

# Function to render a PNG at SIZE from an SVG
render_png() {
  local svg_path="$1"
  local size="$2"
  local png_path="$3"

  if $have_rsvg; then
    rsvg-convert -w "$size" -h "$size" -o "$png_path" "$svg_path"
  else
    # Inkscape CLI (1.0+ syntax)
    inkscape "$svg_path" --export-type=png --export-filename="$png_path" -w "$size" -h "$size" >/dev/null
  fi
}

# Work directory for generated colored SVGs before copying
TMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

echo "Generating colored variants and PNGs..."
for name in "${!COLORS[@]}"; do
  color="${COLORS[$name]}"
  base_out_name="voice-input-${name}"
  tmp_svg="${TMP_DIR}/${base_out_name}.svg"

  # Replace currentColor with the chosen color (case-sensitive, exact token)
  # For base (no-translate) variants, remove the T overlay block before coloring.
  sed '/T_OVERLAY_START/,/T_OVERLAY_END/d' "$SRC_SVG" | sed 's/currentColor/'"$color"'/g' > "$tmp_svg"

  # Save the colored SVG into scalable/apps
  out_svg="${OUT_ROOT}/scalable/apps/${base_out_name}.svg"
  cp "$tmp_svg" "$out_svg"

  # Render PNGs for all sizes
  for sz in "${SIZES[@]}"; do
    out_png="${OUT_ROOT}/${sz}x${sz}/apps/${base_out_name}.png"
    render_png "$tmp_svg" "$sz" "$out_png"
    echo "  -> ${sz}x${sz} ${base_out_name}.png"
  done

  # Generate TRANSLATE variant (with letter 'T' on the left). Keep the overlay and just colorize.
  translate_base_name="voice-input-translate-${name}"
  tmp_svg_translate="${TMP_DIR}/${translate_base_name}.svg"

  sed 's/currentColor/'"$color"'/g' "$SRC_SVG" > "$tmp_svg_translate"

  out_svg_translate="${OUT_ROOT}/scalable/apps/${translate_base_name}.svg"
  cp "$tmp_svg_translate" "$out_svg_translate"
  for sz in "${SIZES[@]}"; do
    out_png_translate="${OUT_ROOT}/${sz}x${sz}/apps/${translate_base_name}.png"
    render_png "$tmp_svg_translate" "$sz" "$out_png_translate"
    echo "  -> ${sz}x${sz} ${translate_base_name}.png"
  done
done

echo "Done."
echo "SVGs: ${OUT_ROOT}/scalable/apps/voice-input-{blue,red,yellow,white}.svg and voice-input-translate-{blue,red,yellow,white}.svg"
echo "PNGs: ${OUT_ROOT}/{16x16,22x22,24x24,32x32,48x48}/apps/{voice-input-*,voice-input-translate-*}.png"
