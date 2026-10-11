#!/bin/sh
set -eu

PROJECT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -p "$PROJECT_DIR/assets"

magick -background none "$PROJECT_DIR/artwork/alien-radio-chrome.svg" -resize 720x560\! "PNG32:$PROJECT_DIR/assets/alien-radio-chrome.png"
magick "$PROJECT_DIR/assets/alien-radio-chrome.png" \( +clone -alpha extract -threshold 10% \) -alpha off -compose CopyOpacity -composite "PNG32:$PROJECT_DIR/assets/window-mask.png"
