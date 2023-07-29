// #![cfg(target_arch = "wasm32")]
// use wasm_bindgen_test::*;

// // #[macro_use]
// // extern crate wasm_bindgen_test;

// #[cfg(test)]
// mod tests {
//     use js_sys::Uint8Array;
//     use serde_wasm_bindgen::from_value;
//     use shinkai_message_wasm::shinkai_message::shinkai_message::{Body, ExternalMetadata, ShinkaiMessage};
//     use shinkai_message_wasm::shinkai_utils::encryption::{
//         encryption_public_key_to_jsvalue, encryption_secret_key_to_jsvalue, unsafe_deterministic_encryption_keypair,
//         EncryptionMethod,
//     };
//     use shinkai_message_wasm::shinkai_utils::signatures::{
//         signature_secret_key_to_jsvalue, unsafe_deterministic_signature_keypair, verify_signature,
//     };
//     use shinkai_message_wasm::{ShinkaiMessageBuilderWrapper, ShinkaiMessageWrapper};
//     use wasm_bindgen::prelude::*;
//     use wasm_bindgen_test::*;

//     #[wasm_bindgen_test]
//     fn test_builder_with_all_fields_no_encryption() {
//         let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
//         let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
//         let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

//         let recipient = "@@other_node.shinkai".to_string();
//         let sender = "@@my_node.shinkai".to_string();
//         let scheduled_time = "20230702T20533481345".to_string();

//         let my_encryption_sk_js = encryption_secret_key_to_jsvalue(&my_encryption_sk);
//         // let my_encryption_pk_js = encryption_public_key_to_jsvalue(&my_encryption_pk);

//         let my_identity_sk_js = signature_secret_key_to_jsvalue(&my_identity_sk);
//         // let my_identity_pk_js = signature_public_key_to_jsvalue(&my_identity_pk);

//         let node2_encryption_pk_js = encryption_public_key_to_jsvalue(&node2_encryption_pk);

//         let mut builder =
//             ShinkaiMessageBuilderWrapper::new(my_encryption_sk_js, my_identity_sk_js, node2_encryption_pk_js).unwrap();

//         let _ = builder.body("body content".into());
//         let _ = builder.body_encryption("None".into());
//         let _ = builder.message_schema_type("TextContent".into());
//         let _ = builder.internal_metadata("".into(), "".into(), "".into(), "None".into());
//         let _ = builder.external_metadata_with_schedule(
//             recipient.clone().into(),
//             sender.clone().into(),
//             scheduled_time.clone().into(),
//         );

//         let message_result = builder.build();
//         assert!(message_result.is_ok());

//         let message_jsvalue: JsValue = message_result.unwrap().into();
//         let message: ShinkaiMessageWrapper = serde_wasm_bindgen::from_value(message_jsvalue).unwrap();
//         let body_jsvalue = message.body();
//         let body: Body = serde_wasm_bindgen::from_value(body_jsvalue).unwrap();

//         let internal_metadata = body.internal_metadata.unwrap();
//         let encryption = EncryptionMethod::from_str(&message.encryption());

//         assert_eq!(body.content, "body content");
//         assert_eq!(encryption, EncryptionMethod::None);
//         assert_eq!(internal_metadata.sender_subidentity, "");
//         assert_eq!(internal_metadata.recipient_subidentity, "");
//         assert_eq!(internal_metadata.inbox, "");

//         let external_metadata_jsvalue = message.external_metadata();
//         let external_metadata: ExternalMetadata = serde_wasm_bindgen::from_value(external_metadata_jsvalue).unwrap();

//         assert_eq!(external_metadata.sender, sender);
//         assert_eq!(external_metadata.scheduled_time, scheduled_time);
//         assert_eq!(external_metadata.recipient, recipient);

//         assert_eq!(0, 1);

//         console_log!("message: {:?}", message);

//         // Convert ShinkaiMessageWrapper back to ShinkaiMessage
//         let message_clone_jsvalue = message.to_jsvalue();
//         // let message_clone: ShinkaiMessage = serde_wasm_bindgen::from_value(message_clone_jsvalue).unwrap();
//         // assert!(verify_signature(&my_identity_pk, &message_clone).unwrap())
//     }

//     // More tests, similar to the one above, go here.

//     // #[wasm_bindgen_test]
//     // fn test_builder_missing_fields() {
//     //     // Setup code with keys goes here.

//     //     let mut builder = ShinkaiMessageBuilderWrapper::new(/* Insert your keys here */);
//     //     let message_result = builder.build();
//     //     assert!(message_result.is_err());
//     // }
// }
