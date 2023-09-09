from setuptools import setup
from setuptools_rust import RustExtension

setup(
    name="shinkai_message_wasm",
    version="0.1",
    rust_extensions=[RustExtension("shinkai_message_wasm.shinkai_message_wasm", binding=pyo3)],
    packages=["shinkai_message_wasm"],
    zip_safe=False,
)