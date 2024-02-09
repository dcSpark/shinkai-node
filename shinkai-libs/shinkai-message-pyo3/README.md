## Compile

Make sure you have `maturin` installed on your machine.

You can build your Python library with the following command:

`maturin develop`

If you want to build a wheel (.whl) file that you can distribute and install with pip, you can use the following command:

`maturin build`

Before running these commands, make sure you have activated the Python virtual environment where you want to install the library, if you're using one.

## Run Tests

WIP: `python -m unittest`

## Troubleshooting

In case you run into any issues with compiling the library, please try the following steps.

### MacOS

In case the default installation process on MacOS happens to not work as expected (for example by using `brew install maturin`), please verify if your `Xcode Command Line Tools` are installed correctly. If they aren't, try to run `xcode-select --install` first and make sure the installation is completed before proceeding to `maturin` installation.
Version of `xcode-select` tooling (`package-id: com.apple.pkg.CLTools_Executables`) tested is `15.1`.

If that doesn't work, you can run `Dockerfile.build` file which uses docker to perform library build.
