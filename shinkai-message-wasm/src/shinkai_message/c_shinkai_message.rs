// use wasm_bindgen::prelude::*;

// #[wasm_bindgen]
// extern "C" {
//     pub type ShinkaiMessageJs;

//     #[wasm_bindgen(constructor)]
//     fn new(body: JsValue, external_metadata: JsValue, encryption: String) -> ShinkaiMessageJs;

//     #[wasm_bindgen(method, getter)]
//     fn body(this: &ShinkaiMessageJs) -> JsValue;

//     #[wasm_bindgen(method, setter)]
//     fn set_body(this: &ShinkaiMessageJs, body: JsValue);

//     #[wasm_bindgen(method, getter)]
//     fn external_metadata(this: &ShinkaiMessageJs) -> JsValue;

//     #[wasm_bindgen(method, setter)]
//     fn set_external_metadata(this: &ShinkaiMessageJs, external_metadata: JsValue);

//     #[wasm_bindgen(method, getter)]
//     fn encryption(this: &ShinkaiMessageJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_encryption(this: &ShinkaiMessageJs, s: String);
// }

// #[wasm_bindgen]
// extern "C" {
//     pub type BodyJs;

//     #[wasm_bindgen(constructor)]
//     fn new(content: String, internal_metadata: JsValue) -> BodyJs;

//     #[wasm_bindgen(method, getter)]
//     fn content(this: &BodyJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_content(this: &BodyJs, s: String);

//     #[wasm_bindgen(method, getter)]
//     fn internal_metadata(this: &BodyJs) -> JsValue;

//     #[wasm_bindgen(method, setter)]
//     fn set_internal_metadata(this: &BodyJs, internal_metadata: JsValue);
// }

// #[wasm_bindgen]
// extern "C" {
//     pub type InternalMetadataJs;

//     #[wasm_bindgen(constructor)]
//     fn new(sender_subidentity: String, recipient_subidentity: String, message_schema_type: String, inbox: String, encryption: String) -> InternalMetadataJs;
    
//     #[wasm_bindgen(method, getter)]
//     fn sender_subidentity(this: &InternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_sender_subidentity(this: &InternalMetadataJs, s: String);

//     #[wasm_bindgen(method, getter)]
//     fn recipient_subidentity(this: &InternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_recipient_subidentity(this: &InternalMetadataJs, s: String);

//     #[wasm_bindgen(method, getter)]
//     fn message_schema_type(this: &InternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_message_schema_type(this: &InternalMetadataJs, s: String);

//     #[wasm_bindgen(method, getter)]
//     fn inbox(this: &InternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_inbox(this: &InternalMetadataJs, s: String);

//     #[wasm_bindgen(method, getter)]
//     fn encryption(this: &InternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_encryption(this: &InternalMetadataJs, s: String);
// }

// #[wasm_bindgen]
// extern "C" {
//     pub type ExternalMetadataJs;

//     #[wasm_bindgen(constructor)]
//     fn new(sender: String, recipient: String, scheduled_time: String, signature: String, other: String) -> ExternalMetadataJs;
    
//     #[wasm_bindgen(method, getter)]
//     fn sender(this: &ExternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_sender(this: &ExternalMetadataJs, s: String);

//     #[wasm_bindgen(method, getter)]
//     fn recipient(this: &ExternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_recipient(this: &ExternalMetadataJs, s: String);

//     #[wasm_bindgen(method, getter)]
//     fn scheduled_time(this: &ExternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_scheduled_time(this: &ExternalMetadataJs, s: String);

//     #[wasm_bindgen(method, getter)]
//     fn signature(this: &ExternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_signature(this: &ExternalMetadataJs, s: String);

//     #[wasm_bindgen(method, getter)]
//     fn other(this: &ExternalMetadataJs) -> String;

//     #[wasm_bindgen(method, setter)]
//     fn set_other(this: &ExternalMetadataJs, s: String);
// }
