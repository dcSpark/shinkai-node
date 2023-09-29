use wasm_bindgen_test::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_wasm_bindgen::from_value;
    use shinkai_message_primitives::schemas::inbox_name::InboxName;
    use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
    use shinkai_message_wasm::shinkai_wasm_wrappers::shinkai_job_wrapper::JobCreationWrapper;
    use wasm_bindgen::JsValue;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_job_creation_wrapper_empty() {
        let job_creation_wrapper = JobCreationWrapper::empty().unwrap();

        // Check that the fields are correctly initialized
        let scope_jsvalue = job_creation_wrapper.get_scope().unwrap();
        let scope: JobScope = from_value(scope_jsvalue).unwrap();
    }

    // #[cfg(target_arch = "wasm32")]
    // #[wasm_bindgen_test]
    // fn test_job_scope_wrapper_new() {
    //     let buckets_js: JsValue = serde_wasm_bindgen::to_value(&vec![
    //         InboxNameWrapper::new(&JsValue::from_str("inbox::@@node1.shinkai::false")).unwrap(),
    //         InboxNameWrapper::new(&JsValue::from_str("inbox::@@node2.shinkai::false")).unwrap()
    //     ]).unwrap();
    //     let documents_js: JsValue = serde_wasm_bindgen::to_value(&vec!["document1".to_string(), "document2".to_string()]).unwrap();
    //     let job_scope_wrapper = JobScopeWrapper::new(&buckets_js, &documents_js).unwrap();

    //     // Check that the fields are correctly initialized
    //     let scope_jsvalue = job_scope_wrapper.to_jsvalue().unwrap();
    //     let scope: JobScope = from_value(scope_jsvalue).unwrap();
    //     assert_eq!(scope.documents, vec!["document1".to_string(), "document2".to_string()]);
    // }
    // #[cfg(target_arch = "wasm32")]
    // #[wasm_bindgen_test]
    // fn test_job_creation_wrapper_from_json_str() {
    //     let json_str = r#"{
    //         "scope": {
    //             "buckets": ["bucket1", "bucket2"],
    //             "documents": ["document1", "document2"]
    //         }
    //     }"#;
    //     let job_creation_wrapper = JobCreationWrapper::from_json_str(json_str).unwrap();

    //     // Check that the fields are correctly converted
    //     let scope_jsvalue = job_creation_wrapper.get_scope().unwrap();
    //     let scope: JobScope = from_value(scope_jsvalue).unwrap();
    // }
}
