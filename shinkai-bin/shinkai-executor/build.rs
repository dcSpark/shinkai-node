use std::{env, path::PathBuf};

fn main() {
    #[cfg(target_os = "linux")]
    let os = "linux";

    #[cfg(target_os = "macos")]
    let os = "mac";

    #[cfg(target_os = "windows")]
    let os = "win";

    #[cfg(target_arch = "aarch64")]
    let arch = "arm64";

    #[cfg(target_arch = "x86_64")]
    let arch = "x64";

    let current_directory = env::var("CARGO_MANIFEST_DIR").unwrap();

    let pdfium_directory = format!("pdfium/{}-{}", os, arch);
    let pdfium_lib_path = PathBuf::from(&current_directory).join(pdfium_directory);

    #[cfg(feature = "static")]
    {
        #[cfg(target_os = "linux")]
        println!("cargo:rustc-link-lib=dylib=stdc++");

        #[cfg(target_os = "macos")]
        {
            println!("cargo:rustc-link-lib=dylib=c++");
            println!("cargo:rustc-link-lib=framework=CoreGraphics");
        }

        println!("cargo:rustc-link-lib=static=pdfium");
        println!("cargo:rustc-link-search=native={}", pdfium_lib_path.display());
    }

    #[cfg(not(feature = "static"))]
    {
        let out_dir = env::var("OUT_DIR").unwrap();
        let out_dir = PathBuf::from(&out_dir);
        let out_dir = out_dir.iter().collect::<Vec<_>>();

        let target_dir = out_dir.iter().take(out_dir.len() - 4).collect::<PathBuf>();
        let bin_dir = target_dir.join(env::var("PROFILE").unwrap());

        #[cfg(target_os = "linux")]
        let pdfium_lib = "libpdfium.so";

        #[cfg(target_os = "macos")]
        let pdfium_lib = "libpdfium.dylib";

        #[cfg(target_os = "windows")]
        let pdfium_lib = "pdfium.dll";

        let pdfium_lib_source = pdfium_lib_path.join(pdfium_lib);
        let pdfium_lib_dest = bin_dir.join(pdfium_lib);

        std::fs::copy(&pdfium_lib_source, &pdfium_lib_dest).unwrap();
    }
}
