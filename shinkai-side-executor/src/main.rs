use image::ImageFormat;
use pdfium_render::prelude::*;

pub fn main() -> Result<(), PdfiumError> {
    // Attempt to bind to a pdfium library in the current working directory; failing that,
    // attempt to bind to a system-provided library.

    // The library name will differ depending on the current platform. On Linux,
    // the library will be named libpdfium.so by default; on Windows, pdfium.dll; and on
    // MacOS, libpdfium.dylib. We can use the Pdfium::pdfium_platform_library_name_at_path()
    // function to append the correct library name for the current platform to a path we specify.

    // === Dynamic linking ===
    let bindings = Pdfium::bind_to_library(
        // Attempt to bind to a pdfium library in the current working directory...
        Pdfium::pdfium_platform_library_name_at_path("./lib"),
    )?;

    let pdfium = Pdfium::new(bindings);

    // This pattern is common enough that it is the default constructor for the Pdfium struct,
    // so we could have also simply written:

    // let pdfium = Pdfium::default();

    // === Static linking ===
    // RUSTFLAGS="-L /path-to/shinkai-node/shinkai-side-executor/lib" cargo build
    // let pdfium = Pdfium::new(Pdfium::bind_to_statically_linked_library().unwrap());

    // Next, we create a set of shared settings that we'll apply to each page in the
    // sample file when rendering. Sharing the same rendering configuration is a good way
    // to ensure homogenous output across all pages in the document.

    let render_config = PdfRenderConfig::new()
        .set_target_width(2000)
        .set_maximum_height(2000)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    // Load the sample file...

    let document = pdfium.load_pdf_from_file("../files/Shinkai_Protocol_Whitepaper.pdf", None)?;

    // ... and export each page to a JPEG in the current working directory,
    // using the rendering configuration we created above.

    for (index, page) in document
        .pages()
        .iter() // ... get an iterator across all pages ...
        .enumerate()
    {
        println!("Exporting page {}", index + 1);
        let result = page
            .render_with_config(&render_config)? // Initializes a bitmap with the given configuration for this page ...
            .as_image() // ... renders it to an Image::DynamicImage ...
            .as_rgba8() // ... sets the correct color space ...
            .ok_or(PdfiumError::ImageError)?
            .save_with_format(format!("output/exported-page-{}.png", index + 1), ImageFormat::Png); // ... and exports it to a JPEG.

        assert!(result.is_ok());
    }

    Ok(())
}
