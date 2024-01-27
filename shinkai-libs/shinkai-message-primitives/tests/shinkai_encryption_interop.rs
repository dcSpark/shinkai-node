#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;
    use shinkai_message_primitives::{
        shinkai_message::shinkai_message::{EncryptedShinkaiBody, EncryptedShinkaiData, MessageBody, MessageData},
        shinkai_utils::encryption::{
            encryption_public_key_to_string, ephemeral_encryption_keys, string_to_encryption_public_key,
            unsafe_deterministic_encryption_keypair,
        },
    };
    use x25519_dalek::{PublicKey, StaticSecret};

    #[test]
    fn test_decrypt_message_body_from_typescript_lib() {
        let encrypted_content = "2c43b44908af3fa56abf4f8aa5b5b5e9913197f2f6825c585701b6fde7d656d98445021736847d807c24073181bedf8684a6835c1353ef704aa7702ea43f55f7e567d8a9bfc01e8aca2f9b80eb4fc7da5c11e702db7e24465288dcc77f00fc44ab09ec0611aec1be14c15ba09c31a09cc251133265f6f64bd63bd6ae5c4ffd9c17cae24aa59a1f147b66e7c61e8ed83bde538df51c941f71838877457174b8421ab8a2979799cedad5e3e7c1fabebde154d99f020d75eeafb69b6e8c0f14a4458126b5ad802ba7188e5d0047fc0c02790ef3b1cbaf102c5905e71996ddc2fea2d28644595655eed02d7e2a23712ae902e0f9487a285222dff8c22788544aa8f6203b78be35c14358e4a1d6af6ce7b659e57161a9d714c60ec254267906ce5f47230718d81a19e35c68f220b73c213e854c5cd5ba03a7c0d0ba7caa9a1c65d63b9d3f863a89f5558808ce953e8caeffc12d0e8f136af1fc31698587b947b654c50b58fc9635a519b73ba68f5ec7ab75580d87f77ea7a2ecbad4877eca002e496930ea8eaf1ac0739bbf826e05016d933b88663908eacc59cdf8da60a39a0b98e33abe0f9d1012971027d6ed31b515812b1b31b010a731fbf06da95e93081680e8ddb16cd6d04cbd3d7bc523";
        let encrypted_data = EncryptedShinkaiBody {
            content: format!("encrypted:{}", encrypted_content),
        };

        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
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

        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        // let pk_string = encryption_public_key_to_string(my_encryption_pk);
        let sender_pk =
            string_to_encryption_public_key("3139fa22bc37ea6266d72a696a930777dec58123254f4a8ab41724421adb2949")
                .unwrap();

        let result = MessageData::decrypt_message_data(&encrypted_data, &my_encryption_sk, &sender_pk);
        eprintln!("result: {:?}", result);
        assert!(result.is_ok());
    }
}
