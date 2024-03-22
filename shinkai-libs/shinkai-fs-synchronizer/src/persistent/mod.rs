use crate::synchronizer::{LocalOSFolderPath, SyncingFolder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryInto;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct StorageData {
    pub sync_folders: HashMap<LocalOSFolderPath, SyncingFolder>,
}

pub struct Storage {
    pub path: String,
    pub file_name: String,
}

impl Storage {
    pub fn new(path: String, file_name: String) -> Self {
        Self { path, file_name }
    }

    pub fn write_sync_folders(&self, data: HashMap<LocalOSFolderPath, SyncingFolder>) -> io::Result<()> {
        let file_path = Path::new(&self.path).join(&self.file_name);
        let file = File::create(file_path)?;
        serde_json::to_writer(file, &data)?;
        Ok(())
    }

    pub fn read_sync_folders(&self) -> io::Result<HashMap<LocalOSFolderPath, SyncingFolder>> {
        let file_path = Path::new(&self.path).join(&self.file_name);
        let mut file = File::open(file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let data: HashMap<LocalOSFolderPath, SyncingFolder> = serde_json::from_str(&contents)?;
        Ok(data)
    }
}
