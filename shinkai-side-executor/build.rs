fn main() {
    #[cfg(feature = "static")]
    {
        use std::env;

        #[cfg(target_os = "macos")]
        println!("cargo:rustc-link-lib=framework=CoreGraphics");

        if let Ok(current_directory) = env::current_dir() {
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

            let pdfium_directory = format!("pdfium/{}-{}", os, arch);
            let pdfium_static_lib_path = current_directory.join(pdfium_directory);

            println!("cargo:rustc-link-search=native={}", pdfium_static_lib_path.display());
            println!("cargo:rustc-link-lib=static=pdfium");

            #[cfg(target_os = "macos")]
            println!("cargo:rustc-link-lib=dylib=c++");

            #[cfg(target_os = "linux")]
            println!("cargo:rustc-link-lib=dylib=stdc++");
        }
    }
}
