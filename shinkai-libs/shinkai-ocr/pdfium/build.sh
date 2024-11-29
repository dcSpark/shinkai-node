#!/bin/bash -eu

OS_NAMES="linux|mac|win"
CPU_NAMES="arm64|x64"

if [[ $# -lt 2 ]]
then
  echo "PDFium build script.
Usage $0 os cpu

Arguments:
   os  = Target OS ($OS_NAMES)
   cpu = Target CPU ($CPU_NAMES)"
  exit
fi

if [[ ! $1 =~ ^($OS_NAMES)$ ]]
then
  echo "Unknown OS: $1"
  exit 1
fi

if [[ ! $2 =~ ^($CPU_NAMES)$ ]]
then
  echo "Unknown CPU: $2"
  exit 1
fi

## Environment

TARGET_OS=$1
TARGET_CPU=$2
TARGET_LINKING=$3

mkdir -p pdfium-source
cd pdfium-source

## Install
if [[ ${4-} != "no-install" ]]
then
  case "$TARGET_OS" in
    linux)
      sudo apt-get update
      sudo apt-get install -y cmake pkg-config g++
      ;;
  esac
fi


# Clone depot tools, standard tools used for building Chromium and associated projects.
if [ ! -d "depot_tools" ]; then
  git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
fi

export PATH="$PATH:$(cd depot_tools; pwd)"

## Checkout

PDFIUM_BRANCH=$(git ls-remote --sort version:refname --refs https://pdfium.googlesource.com/pdfium.git 'chromium/*' | tail -n 1 | cut -d/ -f3-4)

echo "Checking out branch $PDFIUM_BRANCH"

gclient config --unmanaged https://pdfium.googlesource.com/pdfium.git
gclient sync -r "origin/${PDFIUM_BRANCH}" --no-history

## Install dependencies
if [[ ${3-} != "no-install" ]]
then
  case "$TARGET_OS" in
    linux)
      ./pdfium/build/install-build-deps.sh
      gclient runhooks
      ;;
  esac
fi

## Install sysroot
if [[ $TARGET_CPU == "arm64" ]]
then
  python3 ./pdfium/build/linux/sysroot_scripts/install-sysroot.py --arch=arm64
fi

## Configure build

BUILD_TARGET_DIR=out/$TARGET_OS-$TARGET_CPU

cd pdfium
rm -rf $BUILD_TARGET_DIR
gn gen $BUILD_TARGET_DIR

cp ../../args.gn $BUILD_TARGET_DIR/args.gn

(
  cd $BUILD_TARGET_DIR
  echo "target_os = \"$TARGET_OS\"" >> args.gn
  echo "target_cpu = \"$TARGET_CPU\"" >> args.gn

  case "$TARGET_LINKING" in
    static)
      echo "pdf_is_complete_lib = true" >> args.gn
      echo "is_component_build = false" >> args.gn
      ;;
    dynamic)
      echo "pdf_is_complete_lib = false" >> args.gn
      echo "is_component_build = true" >> args.gn
      ;;
  esac

  case "$TARGET_OS" in
    linux | mac)
      echo "clang_use_chrome_plugins = false" >> args.gn
      echo "use_custom_libcxx = false" >> args.gn
      ;;
  esac
)

## Run the build
ninja -C $BUILD_TARGET_DIR pdfium

## Grab the static library
mkdir -p ../../$TARGET_OS-$TARGET_CPU

case "$TARGET_OS" in
  linux | mac)
    case "$TARGET_LINKING" in
      static)
        mv -f $BUILD_TARGET_DIR/obj/libpdfium.a ../../$TARGET_OS-$TARGET_CPU/libpdfium.a
        ;;
      dynamic)
        mv -f $BUILD_TARGET_DIR/libpdfium.so ../../$TARGET_OS-$TARGET_CPU/libpdfium.so
        ;;
    esac
    ;;
  win)
    mv -f $BUILD_TARGET_DIR/obj/pdfium.lib ../../$TARGET_OS-$TARGET_CPU/pdfium.lib
    ;;
esac
