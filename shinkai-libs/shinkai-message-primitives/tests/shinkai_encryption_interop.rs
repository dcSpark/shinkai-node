#[cfg(test)]
mod tests {

    use shinkai_message_primitives::{
        shinkai_message::shinkai_message::{EncryptedShinkaiBody, EncryptedShinkaiData, MessageBody, MessageData},
        shinkai_utils::encryption::{string_to_encryption_public_key, unsafe_deterministic_encryption_keypair},
    };

    #[test]
    fn test_decrypt_message_body_from_typescript_lib() {
        let encrypted_content = "2c43b44908af3fa56abf4f8aa5b5b5e9913197f2f6825c585701b6fde7d656d98445021736847d807c24073181bedf8684a6835c1353ef704aa7702ea43f55f7e567d8a9bfc01e8aca2f9b80eb4fc7da5c11e702db7e24465288dcc77f00fc44ab09ec0611aec1be14c15ba09c31a09cc251133265f6f64bd63bd6ae5c4ffd9c17cae24aa59a1f147b66e7c61e8ed83bde538df51c941f71838877457174b8421ab8a2979799cedad5e3e7c1fabebde154d99f020d75eeafb69b6e8c0f14a4458126b5ad802ba7188e5d0047fc0c02790ef3b1cbaf102c5905e71996ddc2fea2d28644595655eed02d7e2a23712ae902e0f9487a285222dff8c22788544aa8f6203b78be35c14358e4a1d6af6ce7b659e57161a9d714c60ec254267906ce5f47230718d81a19e35c68f220b73c213e854c5cd5ba03a7c0d0ba7caa9a1c65d63b9d3f863a89f5558808ce953e8caeffc12d0e8f136af1fc31698587b947b654c50b58fc9635a519b73ba68f5ec7ab75580d87f77ea7a2ecbad4877eca002e496930ea8eaf1ac0739bbf826e05016d933b88663908eacc59cdf8da60a39a0b98e33abe0f9d1012971027d6ed31b515812b1b31b010a731fbf06da95e93081680e8ddb16cd6d04cbd3d7bc523";
        let encrypted_data = EncryptedShinkaiBody {
            content: format!("encrypted:{}", encrypted_content),
        };

        let (my_encryption_sk, _my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        // let pk_string = encryption_public_key_to_string(my_encryption_pk);
        let sender_pk =
            string_to_encryption_public_key("3139fa22bc37ea6266d72a696a930777dec58123254f4a8ab41724421adb2949")
                .unwrap();

        let result = MessageBody::decrypt_message_body(&encrypted_data, &my_encryption_sk, &sender_pk);
        eprintln!("result: {:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_decrypt_message_data_from_typescript_lib() {
        let encrypted_content = "09000000000000000b00000000000000b3d02cb3feee9c908a1092520af530ac8faef003b8f07c5549924ad4cc19490325be25c6ef256015dcfbf98d4a376f2a";
        let encrypted_data = EncryptedShinkaiData {
            content: format!("encrypted:{}", encrypted_content),
        };

        let (my_encryption_sk, _my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        // let pk_string = encryption_public_key_to_string(my_encryption_pk);
        let sender_pk =
            string_to_encryption_public_key("3139fa22bc37ea6266d72a696a930777dec58123254f4a8ab41724421adb2949")
                .unwrap();

        let result = MessageData::decrypt_message_data(&encrypted_data, &my_encryption_sk, &sender_pk);
        eprintln!("result: {:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_decrypt_new_message_body() {
        let encrypted_content = "c2e96368f4167f1b39d3ec84b1894299067e8f8b279f1c706d48867d736158e18b298abbba124e759dbb1411682812d6583ccbff53feb6f912fe25eeb7c1826441e50ba08c4073b32574dbe7e62ffec024337c5d3caa410fda7294b98f92334b6ac0460e6255ba89b5e596f095bf81e1cab8636040fb5722b684e018fa4a3d0c9cd2f3345e0f2aad6e149b79d6f16274b7353b620c25f5ee6430b9a3cfee69e062de1135d76b61badf667a969d979f17040e96172ec9ef68340d4a912dc4b9fec46fe40c4c9abebc783667c761431e475d9f6d5ead682516d09d811bb8e376385daa163a33fd92fecf5181350fe8ec40cc7a58f5bd40b1de680e157aa23e6a75b23a7b991670e8301d406fdd4d41669377ef970cbe2936166f96bcc5e06428e5716b327088ccd2914c0b990482698498bf4db8b50608104e56249aa210d90fa2055f659b35c45216c697aadc7b8519e5ad51bb694562984f6aad93d2653c3e8186261377a1e6143f6e93bc0d68269276f60e45f3b2b1360b7b3a1a9312bed97df1608ff5a2c934e0917e8a06ca178bfd642cf080a92b485f89658c4dabf4c8b9741de258054cd5d0fcafc4104356325cb66bb2e967d658c4a4eb707769d04bd840a6a8aef141d4e3fb9f26d0";
        let encrypted_data = EncryptedShinkaiBody {
            content: format!("encrypted:{}", encrypted_content),
        };

        let (my_encryption_sk, _my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let sender_pk =
            string_to_encryption_public_key("3139fa22bc37ea6266d72a696a930777dec58123254f4a8ab41724421adb2949")
                .unwrap();

        let result = MessageBody::decrypt_message_body(&encrypted_data, &my_encryption_sk, &sender_pk);
        eprintln!("result: {:?}", result);
        assert!(result.is_ok());
    }

    // Note(Nico): this was used to generate the encrypted content for the typescript lib test
    // #[test]
    // fn test_create_get_all_inboxes_for_profile_request() {
    //     let my_encryption_secret_key = unsafe_deterministic_encryption_keypair(0).0;
    //     let my_signature_secret_key = unsafe_deterministic_signature_keypair(0).0;
    //     let receiver_pk =
    //         string_to_encryption_public_key("3139fa22bc37ea6266d72a696a930777dec58123254f4a8ab41724421adb2949")
    //             .unwrap();

    //     let sender = "@@localhost.shinkai".to_string();
    //     let recipient = "@@localhost.shinkai".to_string();
    //     let sender_subidentity = "main".to_string();

    //     let get_all_inboxes_message_result = ShinkaiMessageBuilder::get_all_inboxes_for_profile(
    //         my_encryption_secret_key,
    //         my_signature_secret_key,
    //         receiver_pk,
    //         format!("{}/{}", sender, sender_subidentity),
    //         sender_subidentity.clone(),
    //         sender.clone(),
    //         recipient.clone(),
    //     );

    //     assert!(get_all_inboxes_message_result.is_ok());
    //     let get_all_inboxes_message = get_all_inboxes_message_result.unwrap();

    //     let message = get_all_inboxes_message.to_string();

    //     eprintln!("get_all_inboxes_message: {:?}", message);
    // }
}
