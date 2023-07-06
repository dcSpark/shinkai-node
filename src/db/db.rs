use crate::{
    shinkai_message::{shinkai_message_handler::ShinkaiMessageHandler, encryption::{string_to_encryption_public_key, encryption_public_key_to_string}, signatures::{string_to_signature_public_key, signature_public_key_to_string}},
    shinkai_message_proto::ShinkaiMessage, network::subidentities::SubIdentity,
};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use prost::Message;
use rand::RngCore;
use rocksdb::{ColumnFamilyDescriptor, Error, Options, DB};
use std::{convert::TryInto, fmt};

#[derive(Debug)]
pub enum ShinkaiMessageDBError {
    RocksDBError(rocksdb::Error),
    DecodeError(prost::DecodeError),
    MessageNotFound,
    CodeAlreadyUsed,
    CodeNonExistent,
    ProfileNameAlreadyExists,
    SomeError,
    ProfileNameNonExistent,
    EncryptionKeyNonExistent,
    PublicKeyParseError,

}

impl From<rocksdb::Error> for ShinkaiMessageDBError {
    fn from(error: rocksdb::Error) -> Self {
        ShinkaiMessageDBError::RocksDBError(error)
    }
}

impl From<prost::DecodeError> for ShinkaiMessageDBError {
    fn from(error: prost::DecodeError) -> Self {
        ShinkaiMessageDBError::DecodeError(error)
    }
}

#[derive(PartialEq)]
pub enum RegistrationCodeStatus {
    Unused,
    Used,
}

impl RegistrationCodeStatus {
    pub fn from_slice(slice: &[u8]) -> Self {
        match slice {
            b"unused" => Self::Unused,
            _ => Self::Used,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Unused => b"unused",
            Self::Used => b"used",
        }
    }
}

pub enum Topic {
    Peers,
    ProfilesIdentityKey,
    ProfilesEncryptionKey,
    ScheduledMessage,
    AllMessages,
    AllMessagesTimeKeyed,
    OneTimeRegistrationCodes,
}

impl Topic {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Peers => "peers",
            Self::ProfilesIdentityKey => "profiles_identity_key",
            Self::ProfilesEncryptionKey => "profiles_encryption_key",
            Self::ScheduledMessage => "scheduled_message",
            Self::AllMessages => "all_messages",
            Self::AllMessagesTimeKeyed => "all_messages_time_keyed",
            Self::OneTimeRegistrationCodes => "one_time_registration_codes",
        }
    }
}

pub struct ShinkaiMessageDB {
    db: DB,
    pub path: String,
}

impl ShinkaiMessageDB {
    pub fn new(db_path: &str) -> Result<Self, Error> {
        let cf_names = vec![
            Topic::Peers.as_str(),
            Topic::ProfilesEncryptionKey.as_str(),
            Topic::ProfilesIdentityKey.as_str(),
            Topic::ScheduledMessage.as_str(),
            Topic::AllMessages.as_str(),
            Topic::AllMessagesTimeKeyed.as_str(),
            Topic::OneTimeRegistrationCodes.as_str(),
        ];

        let mut cfs = vec![];
        for cf_name in &cf_names {
            // Create Options for ColumnFamily
            let mut cf_opts = Options::default();
            cf_opts.create_if_missing(true);
            cf_opts.create_missing_column_families(true);

            // Create ColumnFamilyDescriptor for each ColumnFamily
            // println!("Creating ColumnFamily: {}", cf_name);
            let cf_desc = ColumnFamilyDescriptor::new(cf_name.to_string(), cf_opts);
            cfs.push(cf_desc);
        }

        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        let db = DB::open_cf_descriptors(&db_opts, db_path, cfs)?;

        Ok(ShinkaiMessageDB {
            db,
            path: db_path.to_string(),
        })
    }

    pub fn insert(&self, key: String, message: &ShinkaiMessage, topic: Topic) -> Result<(), Error> {
        // As protobuf uses bytes to serialize data, we can use this to store into RocksDB
        let message_bytes = ShinkaiMessageHandler::encode_message(message.clone());
        let cf = self.db.cf_handle(topic.as_str()).unwrap();
        self.db.put_cf(cf, key, message_bytes)
    }

    pub fn get(&self, key: String, topic: Topic) -> Result<Option<ShinkaiMessage>, Error> {
        let cf = self.db.cf_handle(topic.as_str()).unwrap();
        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let message = ShinkaiMessageHandler::decode_message(bytes.to_vec()).unwrap();
                Ok(Some(message))
            }
            None => Ok(None),
        }
    }

    pub fn write_to_peers(&self, key: &str, address: &str) -> Result<(), Error> {
        let cf = self.db.cf_handle(Topic::Peers.as_str()).unwrap();
        self.db.put_cf(cf, key, address.as_bytes())
    }

    pub fn get_all_peers(&self) -> Result<Vec<(String, String)>, Error> {
        let cf = self.db.cf_handle(Topic::Peers.as_str()).unwrap();
        let mut result = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            // Handle the Result returned by the iterator
            match item {
                Ok((key, value)) => {
                    let key_str = String::from_utf8(key.to_vec()).unwrap();
                    let value_str = String::from_utf8(value.to_vec()).unwrap();
                    result.push((key_str, value_str));
                }
                Err(e) => return Err(e),
            }
        }

        Ok(result)
    }

    pub fn insert_message(&self, message: &ShinkaiMessage) -> Result<(), Error> {
        // Calculate the hash of the message for the key
        let hash_key = ShinkaiMessageHandler::calculate_hash(&message);

        // Clone the external_metadata first, then unwrap
        let cloned_external_metadata = message.external_metadata.clone();
        let ext_metadata = cloned_external_metadata.unwrap();

        // Calculate the scheduled time or current time
        let time_key = match ext_metadata.scheduled_time.is_empty() {
            true => ShinkaiMessageHandler::generate_time_now(),
            false => ext_metadata.scheduled_time.clone(),
        };

        // Create a write batch
        let mut batch = rocksdb::WriteBatch::default();

        // Define the data for AllMessages
        let all_messages_cf = self.db.cf_handle(Topic::AllMessages.as_str()).unwrap();
        let message_bytes = ShinkaiMessageHandler::encode_message(message.clone());
        batch.put_cf(all_messages_cf, &hash_key, &message_bytes);

        // Define the data for AllMessagesTimeKeyed
        let all_messages_time_keyed_cf = self
            .db
            .cf_handle(Topic::AllMessagesTimeKeyed.as_str())
            .unwrap();
        batch.put_cf(all_messages_time_keyed_cf, &time_key, &hash_key);

        // Atomically apply the updates
        self.db.write(batch)?;

        Ok(())
    }

    pub fn schedule_message(&self, message: &ShinkaiMessage) -> Result<(), Error> {
        // Calculate the scheduled time or current time
        let time_key = match message
            .external_metadata
            .clone()
            .unwrap()
            .scheduled_time
            .is_empty()
        {
            true => ShinkaiMessageHandler::generate_time_now(),
            false => message
                .external_metadata
                .clone()
                .unwrap()
                .scheduled_time
                .clone(),
        };

        // Convert ShinkaiMessage into bytes for storage
        let message_bytes = ShinkaiMessageHandler::encode_message(message.clone());

        // Retrieve the handle to the "ToSend" column family
        let to_send_cf = self.db.cf_handle(Topic::ScheduledMessage.as_str()).unwrap();

        // Insert the message into the "ToSend" column family using the time key
        self.db.put_cf(to_send_cf, time_key, message_bytes)?;

        Ok(())
    }

    pub fn get_last_messages(
        &self,
        n: usize,
    ) -> Result<Vec<ShinkaiMessage>, ShinkaiMessageDBError> {
        let time_keyed_cf = self
            .db
            .cf_handle(Topic::AllMessagesTimeKeyed.as_str())
            .unwrap();
        let messages_cf = self.db.cf_handle(Topic::AllMessages.as_str()).unwrap();

        let iter = self
            .db
            .iterator_cf(time_keyed_cf, rocksdb::IteratorMode::End);

        let mut messages = Vec::new();
        for item in iter.take(n) {
            // Handle the Result returned by the iterator
            match item {
                Ok((_, value)) => {
                    // The value of the AllMessagesTimeKeyed CF is the key in the AllMessages CF
                    let message_key = value.to_vec();

                    // Fetch the message from the AllMessages CF
                    match self.db.get_cf(messages_cf, &message_key)? {
                        Some(bytes) => {
                            let message = ShinkaiMessageHandler::decode_message(bytes.to_vec())?;
                            messages.push(message);
                        }
                        None => return Err(ShinkaiMessageDBError::MessageNotFound),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(messages)
    }

    pub fn generate_registration_new_code(&self) -> Result<String, Error> {
        let mut rng = rand::thread_rng();
        let mut random_bytes = [0u8; 64];
        rng.fill_bytes(&mut random_bytes);
        let new_code = bs58::encode(random_bytes).into_string();

        let cf = self.db.cf_handle(Topic::OneTimeRegistrationCodes.as_str()).unwrap();
        self.db.put_cf(cf, &new_code, b"unused")?;
        
        Ok(new_code)
    }

    pub fn use_registration_code(&self, registration_code: &str, identity_public_key: &str, encryption_public_key: &str, profile_name: &str) -> Result<(), ShinkaiMessageDBError> {
        // Check if the code exists in Topic::OneTimeRegistrationCodes and its value is unused
        let cf_codes = self.db.cf_handle(Topic::OneTimeRegistrationCodes.as_str()).unwrap();
        match self.db.get_cf(cf_codes, registration_code)? {
            Some(value) => {
                if RegistrationCodeStatus::from_slice(&value) != RegistrationCodeStatus::Unused {
                    return Err(ShinkaiMessageDBError::CodeAlreadyUsed);
                }
            }
            None => return Err(ShinkaiMessageDBError::CodeNonExistent),
        }
    
        // Check that the profile name doesn't exist in ProfilesIdentityKey and ProfilesEncryptionKey
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        if self.db.get_cf(cf_identity, profile_name)?.is_some() {
            return Err(ShinkaiMessageDBError::ProfileNameAlreadyExists);
        }
        
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        if self.db.get_cf(cf_encryption, profile_name)?.is_some() {
            return Err(ShinkaiMessageDBError::ProfileNameAlreadyExists);
        }
        
        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();
    
        // Mark the registration code as used
        batch.put_cf(cf_codes, registration_code, RegistrationCodeStatus::Used.as_bytes());
    
        // Write to ProfilesIdentityKey and ProfilesEncryptionKey
        batch.put_cf(cf_identity, profile_name, identity_public_key.as_bytes());
        batch.put_cf(cf_encryption, profile_name, encryption_public_key.as_bytes());
    
        // Write the batch
        self.db.write(batch)?;
    
        Ok(())
    }

    pub fn get_encryption_public_key(&self, identity_public_key: &str) -> Result<String, ShinkaiMessageDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        
        // Get the associated profile name for the identity public key
        let profile_name = match self.db.get_cf(cf_identity, identity_public_key)? {
            Some(name_bytes) => Ok(String::from_utf8_lossy(&name_bytes).to_string()),
            None => Err(ShinkaiMessageDBError::ProfileNameNonExistent),
        }?;
    
        // Get the associated encryption public key for the profile name
        match self.db.get_cf(cf_encryption, &profile_name)? {
            Some(encryption_key_bytes) => Ok(String::from_utf8_lossy(&encryption_key_bytes).to_string()),
            None => Err(ShinkaiMessageDBError::EncryptionKeyNonExistent),
        }
    }

    pub fn load_all_sub_identities(&self) -> Result<Vec<(String, EncryptionPublicKey, SignaturePublicKey)>, ShinkaiMessageDBError> {
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
    
        let mut result = Vec::new();
    
        let iter = self.db.iterator_cf(cf_encryption, rocksdb::IteratorMode::Start);
        for item in iter {
            // Handle the Result returned by the iterator
            match item {
                Ok((key, value)) => {
                    let name = String::from_utf8(key.to_vec()).unwrap();
                    let encryption_public_key = string_to_encryption_public_key(&String::from_utf8(value.to_vec()).unwrap())
                        .map_err(|_| ShinkaiMessageDBError::PublicKeyParseError)?;
    
                    // get the associated signature public key
                    match self.db.get_cf(cf_identity, &name)? {
                        Some(value) => {
                            let signature_public_key = string_to_signature_public_key(&String::from_utf8(value.to_vec()).unwrap())
                                .map_err(|_| ShinkaiMessageDBError::PublicKeyParseError)?;
                            result.push((name, encryption_public_key, signature_public_key));
                        }
                        None => return Err(ShinkaiMessageDBError::ProfileNameNonExistent),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
    
        Ok(result)
    }    

    pub fn remove_identity(&self, name: &str) -> Result<(), ShinkaiMessageDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        
        // Check that the profile name exists in ProfilesIdentityKey and ProfilesEncryptionKey
        if self.db.get_cf(cf_identity, name)?.is_none() || self.db.get_cf(cf_encryption, name)?.is_none() {
            return Err(ShinkaiMessageDBError::ProfileNameNonExistent);
        }
    
        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();
    
        // Delete from ProfilesIdentityKey and ProfilesEncryptionKey
        batch.delete_cf(cf_identity, name);
        batch.delete_cf(cf_encryption, name);
    
        // Write the batch
        self.db.write(batch)?;
    
        Ok(())
    }

    pub fn insert_sub_identity(&self, identity: SubIdentity) -> Result<(), ShinkaiMessageDBError> {
        let cf_identity = self.db.cf_handle(Topic::ProfilesIdentityKey.as_str()).unwrap();
        let cf_encryption = self.db.cf_handle(Topic::ProfilesEncryptionKey.as_str()).unwrap();
        
        // Check that the profile name doesn't exist in ProfilesIdentityKey and ProfilesEncryptionKey
        if self.db.get_cf(cf_identity, &identity.name)?.is_some() || self.db.get_cf(cf_encryption, &identity.name)?.is_some() {
            return Err(ShinkaiMessageDBError::ProfileNameAlreadyExists);
        }
    
        // Start write batch for atomic operation
        let mut batch = rocksdb::WriteBatch::default();
    
        // Write to ProfilesIdentityKey and ProfilesEncryptionKey
        
        batch.put_cf(cf_identity, &identity.name, signature_public_key_to_string(identity.signature_public_key).as_bytes());
        batch.put_cf(cf_encryption, &identity.name, encryption_public_key_to_string(identity.encryption_public_key).as_bytes());
    
        // Write the batch
        self.db.write(batch)?;
    
        Ok(())
    }
        
}

impl fmt::Display for ShinkaiMessageDBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShinkaiMessageDBError::RocksDBError(e) => write!(f, "RocksDB error: {}", e),
            ShinkaiMessageDBError::CodeAlreadyUsed => write!(f, "Registration code has already been used"),
            ShinkaiMessageDBError::CodeNonExistent => write!(f, "Registration code does not exist"),
            ShinkaiMessageDBError::ProfileNameAlreadyExists => write!(f, "Profile name already exists"),
            ShinkaiMessageDBError::DecodeError(e) => write!(f, "Decoding Error: {}", e),
            ShinkaiMessageDBError::MessageNotFound => write!(f, "Message not found"),
            ShinkaiMessageDBError::SomeError => write!(f, "Some mysterious error..."),
            ShinkaiMessageDBError::ProfileNameNonExistent => write!(f, "Profile name does not exist"),
            ShinkaiMessageDBError::EncryptionKeyNonExistent => write!(f, "Encryption key does not exist"),
            ShinkaiMessageDBError::PublicKeyParseError => write!(f, "Error parsing public key"),
        }
    }
}

impl std::error::Error for ShinkaiMessageDBError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShinkaiMessageDBError::RocksDBError(e) => Some(e),
            ShinkaiMessageDBError::DecodeError(e) => Some(e),
            _ => None,
        }
    }
}