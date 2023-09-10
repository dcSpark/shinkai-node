from setuptools import setup
from setuptools_rust import RustExtension

setup(
    name="shinkai_message_pyo3",
    version="0.1",
    rust_extensions=[RustExtension("shinkai_message_pyo3.shinkai_message_primitives", binding=pyo3)],
    packages=["shinkai_message_pyo3"],
    zip_safe=False,
)
