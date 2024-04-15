use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct PyFolderSubscription {
    #[pyo3(get, set)]
    pub minimum_token_delegation: Option<u64>,
    #[pyo3(get, set)]
    pub minimum_time_delegated_hours: Option<u64>,
    #[pyo3(get, set)]
    pub monthly_payment: Option<PyPaymentOption>,
    #[pyo3(get, set)]
    pub is_free: bool,
    #[pyo3(get, set)]
    pub folder_description: String,
}

#[pyclass]
#[derive(Clone)]
pub struct PyPaymentOption {
    #[pyo3(get, set)]
    pub usd: Option<f64>,
    #[pyo3(get, set)]
    pub kai_tokens: Option<u64>,
}

#[pymethods]
impl PyPaymentOption {
    #[new]
    pub fn new(usd: Option<f64>, kai_tokens: Option<u64>) -> Self {
        Self { usd, kai_tokens }
    }
}

#[pymethods]
impl PyFolderSubscription {
    #[new]
    #[pyo3(signature = (folder_description, is_free, minimum_token_delegation = None, minimum_time_delegated_hours = None, monthly_payment = None))]
    pub fn new(
        folder_description: String,
        is_free: bool,
        minimum_token_delegation: Option<u64>,
        minimum_time_delegated_hours: Option<u64>,
        monthly_payment: Option<PyPaymentOption>,
    ) -> Self {
        Self {
            minimum_token_delegation,
            minimum_time_delegated_hours,
            monthly_payment,
            is_free,
            folder_description,
        }
    }
}
