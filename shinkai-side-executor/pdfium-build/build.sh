#!/usr/bin/env bash
mkdir -p pdfium
cd pdfium

# Clone depot tools, standard tools used for building Chromium and associated projects.
git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
export PATH="$PATH:$(cd depot_tools; pwd)"

# Clone the pdfium source.
gclient config --unmanaged https://pdfium.googlesource.com/pdfium.git
gclient sync --no-history

# Create default build configuration...
cd pdfium
rm -rf out/lib
gn gen out/lib

cp args.gn out/lib/args.gn

# Run the build.
ninja -C out/lib pdfium

# Grab libpdfium.a from out/lib/obj