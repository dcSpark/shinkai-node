from setuptools import setup
from setuptools_rust import RustExtension

setup(
    name="shinkai_message_pyo3",
    version="0.1",
    author="Nico Arqueros",
    author_email="nico@shinkai.com",
    packages=["shinkai_message_pyo3"],
    rust_extensions=[RustExtension("shinkai_message_pyo3.shinkai_message_pyo3", binding="pyo3")],
    zip_safe=False,
)