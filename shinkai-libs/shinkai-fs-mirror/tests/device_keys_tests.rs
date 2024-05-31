#[cfg(test)]
mod tests {
    use shinkai_fs_mirror::shinkai::{shinkai_device_keys::ShinkaiDeviceKeys, shinkai_utils::decrypt_exported_keys};
    use std::env;

    #[test]
    fn test_decrypt_message_with_passphrase() {
        let unencrypted_keys = ShinkaiDeviceKeys {
            my_device_encryption_pk: "5fd7f565d45cd9245e54b6b54e42c0bc556187962f458385652751fbbc58f079".to_string(),
            my_device_encryption_sk: "08c8e480eece95571fb3655216665ca71b92a64392d1da43555d49b65505bf67".to_string(),
            my_device_identity_pk: "e7b2259353879f1e7f8c421b0f57ffc7aafa7a5f798cf342c995b111567319b9".to_string(),
            my_device_identity_sk: "217448b58fa4f168bd8b8631b62747bf52566e83d14542a3b4cd716822a873b6".to_string(),
            profile_encryption_pk: "1d2638e7cfd21c35a20191f6165eac7d0a5536650d2c27f4abb5e19b60544a33".to_string(),
            profile_encryption_sk: "68baea97ffde694dc1b5ccc13fe88d349926309705ba784b79f5173bd56e147d".to_string(),
            profile_identity_pk: "0a0598c61144a10fb0a1baa32f115b0d22f3a00c93a3cc368779ea82c5ecfc41".to_string(),
            profile_identity_sk: "717eb9b4e3bb681349d2e21dace6424567261d98507696f10923930a1efc0269".to_string(),
            profile: "main".to_string(),
            identity_type: Some("device".to_string()),
            permission_type: "".to_string(),
            shinkai_identity: "@@localhost.shinkai".to_string(),
            registration_code: Some("".to_string()),
            node_encryption_pk: "798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679".to_string(),
            node_address: "http://127.0.0.1:9550".to_string(),
            registration_name: "main_device".to_string(),
            node_signature_pk: "801e158387f6d78ebf39104a964ea1b11b4d3e5269556fb9fd063daf92dfa972".to_string(),
        };

        let encrypted_body = "encrypted:21d785751334d99b97d7ccada979b9b5fbf7dcc89c72363228a7e2312343f8add1fe4bc67324350a2e6425b6de22db878c1c30281cba1440d6a16d6edf9a4aecdacc1716e1f6abde9efd1ba4d5bc94ca4f9cbaed093fc3d738eb8db425da59c68c33d5bce475bdd72e1842db1f5a961a2ee2e6231a6215dd668c0b7f1114f72fc254700be4b9d1300bd0f9d771f80c01ea9f49912e385fb7d1bb395538162f12289c92f01b8c21df3013db78a3bdfb12c6db47b102c4d027c211ac9f9b9e48f51c02eafe440e219ba5345e218e19dfca023b032ac03ef1d797370a36c579cf895c9339f84416fce180c6fa6c225ceeca7dd481b3d46c1256d0947181427bd3db44ad971ce746721ce0d9df68e4392f5b21ed73a7f8a8dbc5903348c0af5cbb98a8836d0a0ca64234f5f0c98db434cdfb26582fe4c022d8e90b5114972bf44d1d4e3088bad3878098aba4e90ae84f5a353fa396a4cd9ffccf3a3333fb2a552e6102100241252ede61e917c658b427ad0750f9a5131d148cd38e2205b9d0c722d8ac0f2394a3a7ed8034c1cdc0f72f6370a3109ad5aa13bfe670fa5af3492151086e1142eaaa4bd360831e85b3e7284e0ab087fbf5359863ecea9d2f8eca9424325a95cb028d9b985e72941ea0391f47287f6e944700e8174a2112535e194b5cfb60ce9d9e0cfb4a6c26eafbc326c29abd3a01a89011628105bce1f81400210ab7be3bcb784a880e35fd684d2d16d8b2a1e193107a3acdb262692ef0270cb6456853952234872f334fc7b31e047ac797c1239880201dcff62c7750699b823e6f85902565ae5a44885046a5310610ac079dac4bc2c3a6a1c210b7935b2857600c14aad71f65b3c8f2c45484c694d5ad703d3db328629ea2d39526aba589af6fc9ac76012fb19133e68336b9ccc7f0764831bafbe7739541f3a816f6404908fa422719cb806da769deea1bd119d5e4596276711f1ee60a4af15b2d6b37d00442ffe78ccbcf4e7aeed45f68f8d9daf5b0e2446f430a3340e3e5b9d504e607c459970e29131467f96fc0f4547b861a48f9374fcdadd8ac4639434b4cc6169b6d2e896ded99456d61b2f41cf7454fb3651816ed3061cf83d145ef42c47579b3252fe62534a0a87371e2c8f680fa80cd6019479a16865f80307500ab8ab6a3ce307566cd78f3d6483fc1a4477b0ed6de4f53e0e2a1d5d27063403af56ca0eb7a8bdb244383d2abd84da8172c2ca936bbe730fa8b2a9fd501f905ca39d5b06cf8b95ed8b046daf55478c91f064a0c029ed62466baf10643a43120c63097f26e84705dba5d160e340a61f99bd970d2c293c3628bf94c6574f4920021bf2718df4a4610985a0f0b97eef66bd2afc8fe40160daab546d9e858a4ea7d2fee99a4ee3960a030c8e99c6680b46fd1233cf9b704108836d75c7b1727a8662cffe1ad82246d3faa559076a5a75fecab14fa1a814fc6c95f0e27d80fcaefcc26c91851fdfb562d94e9a2b9dc2fe4e2a9d314d50c510e023cdc94fcaaa443d540724f44d31d690b73d41a6f6fbf9060dc8c02506f30f89182f256ce96710f1cf56dcaf606ce7227963fa9a2dbb844d49cf3b108dd732f970e27056622e899ee473cda4bd9f6";

        let passphrase = "password123$";

        let decrypted_result = decrypt_exported_keys(encrypted_body, passphrase);
        assert!(
            decrypted_result.is_ok(),
            "Decryption should succeed with the correct passphrase"
        );
        let decrypted_keys = decrypted_result.unwrap();
        assert!(
            !decrypted_keys.my_device_encryption_pk.is_empty(),
            "Decrypted device encryption pk should not be empty"
        );

        assert_eq!(
            decrypted_keys.my_device_encryption_pk, unencrypted_keys.my_device_encryption_pk,
            "Decrypted and unencrypted device encryption pk should match"
        );
        assert_eq!(
            decrypted_keys.my_device_encryption_sk, unencrypted_keys.my_device_encryption_sk,
            "Decrypted and unencrypted device encryption sk should match"
        );
        assert_eq!(
            decrypted_keys.my_device_identity_pk, unencrypted_keys.my_device_identity_pk,
            "Decrypted and unencrypted device identity pk should match"
        );
        assert_eq!(
            decrypted_keys.my_device_identity_sk, unencrypted_keys.my_device_identity_sk,
            "Decrypted and unencrypted device identity sk should match"
        );
        assert_eq!(
            decrypted_keys.profile_encryption_pk, unencrypted_keys.profile_encryption_pk,
            "Decrypted and unencrypted profile encryption pk should match"
        );
        assert_eq!(
            decrypted_keys.profile_encryption_sk, unencrypted_keys.profile_encryption_sk,
            "Decrypted and unencrypted profile encryption sk should match"
        );
        assert_eq!(
            decrypted_keys.profile_identity_pk, unencrypted_keys.profile_identity_pk,
            "Decrypted and unencrypted profile identity pk should match"
        );
        assert_eq!(
            decrypted_keys.profile_identity_sk, unencrypted_keys.profile_identity_sk,
            "Decrypted and unencrypted profile identity sk should match"
        );
        assert_eq!(
            decrypted_keys.profile, unencrypted_keys.profile,
            "Decrypted and unencrypted profile should match"
        );
        assert_eq!(
            decrypted_keys.identity_type, unencrypted_keys.identity_type,
            "Decrypted and unencrypted identity type should match"
        );
        assert_eq!(
            decrypted_keys.permission_type, unencrypted_keys.permission_type,
            "Decrypted and unencrypted permission type should match"
        );
        assert_eq!(
            decrypted_keys.shinkai_identity, unencrypted_keys.shinkai_identity,
            "Decrypted and unencrypted shinkai identity should match"
        );
        assert_eq!(
            decrypted_keys.registration_code, unencrypted_keys.registration_code,
            "Decrypted and unencrypted registration code should match"
        );
        assert_eq!(
            decrypted_keys.node_encryption_pk, unencrypted_keys.node_encryption_pk,
            "Decrypted and unencrypted node encryption pk should match"
        );
        assert_eq!(
            decrypted_keys.node_address, unencrypted_keys.node_address,
            "Decrypted and unencrypted node address should match"
        );
        assert_eq!(
            decrypted_keys.registration_name, unencrypted_keys.registration_name,
            "Decrypted and unencrypted registration name should match"
        );
        assert_eq!(
            decrypted_keys.node_signature_pk, unencrypted_keys.node_signature_pk,
            "Decrypted and unencrypted node signature pk should match"
        );
    }
}
