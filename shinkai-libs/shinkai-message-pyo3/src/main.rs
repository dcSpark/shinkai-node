pub mod shinkai_pyo3_wrapper;
pub mod shinkai_pyo3_utils;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    pyo3::prepare_freethreaded_python();
    Ok(())
}
