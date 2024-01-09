use super::resources_tests::get_shinkai_intro_doc_async;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_node::vector_fs::vector_fs_internals::VectorFSInternals;
use shinkai_node::vector_fs::vector_fs_reader::VFSReader;
use shinkai_node::vector_fs::vector_fs_writer::VFSWriter;
use shinkai_node::vector_fs::{db::fs_db::VectorFSDB, vector_fs::VectorFS, vector_fs_error::VectorFSError};
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::model_type::{EmbeddingModelType, TextEmbeddingsInference};
use shinkai_vector_resources::vector_resource::{
    BaseVectorResource, VRPath, VectorResource, VectorResourceCore, VectorResourceSearch,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

fn default_test_profile() -> ShinkaiName {
    ShinkaiName::new("@@localhost.shinkai/profileName".to_string()).unwrap()
}

fn node_name() -> ShinkaiName {
    ShinkaiName::new("@@localhost.shinkai".to_string()).unwrap()
}

fn setup_default_vec_fs() -> VectorFS {
    let generator = RemoteEmbeddingGenerator::new_default();
    let fs_db_path = format!("db_tests/{}", "vec_fs");
    let profile_list = vec![default_test_profile()];
    let supported_embedding_models = vec![EmbeddingModelType::TextEmbeddingsInference(
        TextEmbeddingsInference::AllMiniLML6v2,
    )];

    VectorFS::new(
        generator,
        supported_embedding_models,
        profile_list,
        &fs_db_path,
        node_name(),
    )
    .unwrap()
}

#[tokio::test]
async fn test_vector_fs_initializes_new_profile_automatically() {
    setup();
    let generator = RemoteEmbeddingGenerator::new_default();
    let mut vector_fs = setup_default_vec_fs();

    let fs_internals = vector_fs._get_profile_fs_internals(&default_test_profile());
    assert!(fs_internals.is_ok())
}

#[tokio::test]
async fn test_vector_fs_saving_reading() {
    setup();
    let generator = RemoteEmbeddingGenerator::new_default();
    let mut vector_fs = setup_default_vec_fs();

    let path = VRPath::new();
    let writer = vector_fs
        .new_writer(default_test_profile(), path.clone(), default_test_profile())
        .unwrap();
    let folder_name = "first_folder";
    vector_fs.create_new_folder(&writer, folder_name);

    // Validate new folder path points to an entry at all (not empty), then specifically a folder, and finally not to an item.
    let folder_path = path.push_cloned(folder_name.to_string());
    assert!(vector_fs
        ._validate_path_points_to_entry(folder_path.clone(), &writer.profile)
        .is_ok());
    assert!(vector_fs
        ._validate_path_points_to_folder(folder_path.clone(), &writer.profile)
        .is_ok());
    assert!(vector_fs
        ._validate_path_points_to_item(folder_path.clone(), &writer.profile)
        .is_err());

    // Create a Vector Resource and source file to be added into the VectorFS
    let (doc_resource, source_file) = get_shinkai_intro_doc_async(&generator, &vec![]).await.unwrap();
    let resource = BaseVectorResource::Document(doc_resource);
    let writer = vector_fs
        .new_writer(default_test_profile(), folder_path.clone(), default_test_profile())
        .unwrap();
    vector_fs
        .save_vector_resource_in_folder(&writer, resource.clone(), Some(source_file))
        .unwrap();

    // Validate new item path points to an entry at all (not empty), then specifically an item, and finally not to a folder.
    let item_path = folder_path.push_cloned(resource.as_trait_object().name().to_string());
    assert!(vector_fs
        ._validate_path_points_to_entry(item_path.clone(), &writer.profile)
        .is_ok());
    assert!(vector_fs
        ._validate_path_points_to_item(item_path.clone(), &writer.profile)
        .is_ok());
    assert!(vector_fs
        ._validate_path_points_to_folder(item_path.clone(), &writer.profile)
        .is_err());

    let internals = vector_fs
        ._get_profile_fs_internals_read_only(&default_test_profile())
        .unwrap();
    internals.fs_core_resource.print_all_nodes_exhaustive(None, true, false);

    /// Retrieve the Vector Resource & Source File from the db
    ///
    // let reader = vector_fs.db
    assert!(1 == 2);
}
