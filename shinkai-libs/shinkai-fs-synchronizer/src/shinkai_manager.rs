use ed25519_dalek::SigningKey;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::{ProfileName, ShinkaiMessageBuilder};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[derive(Clone)]
pub struct ShinkaiManager {
    pub message_builder: ShinkaiMessageBuilder,
    pub my_encryption_secret_key: EncryptionStaticKey,
    pub my_signature_secret_key: SigningKey,
    pub receiver_public_key: EncryptionPublicKey,
    pub sender: ProfileName,
    pub sender_subidentity: String,
    pub node_receiver: ProfileName,
    pub node_receiver_subidentity: ProfileName,
}

impl ShinkaiManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        message_builder: ShinkaiMessageBuilder,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Self {
        Self {
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            message_builder,
        }
    }

    fn check_folder_exists(&mut self, path: &str) -> Result<bool, &'static str> {
        let message = self.message_builder.vecfs_retrieve_path_simplified(
            path,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key.clone(),
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        );

        match message {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    fn create_folder(&mut self, folder_name: &str, path: &str) -> Result<(), &'static str> {
        self.message_builder.vecfs_create_folder(
            folder_name,
            path,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key.clone(),
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        )?;

        Ok(())
    }

    // fn delete_folder(&self, folder_name: &str, path: &str) -> Result<(), &'static str> {
    //     // Assuming there's a method in the message builder for deleting a folder, which is not shown in the provided context
    //     self.message_builder.vecfs_delete_folder(
    //         folder_name,
    //         path,
    //         self.my_encryption_secret_key.clone(),
    //         self.my_signature_secret_key.clone(),
    //         self.receiver_public_key.clone(),
    //         self.sender.clone(),
    //         self.sender_subidentity.clone(),
    //         self.node_receiver.clone(),
    //         self.node_receiver_subidentity.clone(),
    //     )?;

    //     Ok(())
    // }

    // fn upload_file(&self, file_path: &str, destination_path: &str) -> Result<(), &'static str> {
    //     // Assuming there's a method in the message builder for uploading a file, which is not shown in the provided context
    //     self.message_builder.vecfs_upload_file(
    //         file_path,
    //         destination_path,
    //         self.my_encryption_secret_key.clone(),
    //         self.my_signature_secret_key.clone(),
    //         self.receiver_public_key.clone(),
    //         self.sender.clone(),
    //         self.sender_subidentity.clone(),
    //         self.node_receiver.clone(),
    //         self.node_receiver_subidentity.clone(),
    //     )?;

    //     Ok(())
    // }

    fn add_items_to_db(&mut self, destination_path: &str, file_inbox: &str) -> Result<(), &'static str> {
        // Assuming there's a method in the message builder for adding items to a database, which is not shown in the provided context
        self.message_builder.vecfs_create_items(
            destination_path,
            file_inbox,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        )?;

        Ok(())
    }
}
