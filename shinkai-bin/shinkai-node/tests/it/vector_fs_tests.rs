use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit};
use chrono::{TimeZone, Utc};
use mockito::Server;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, Ollama, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIConvertFilesAndSaveToFolder, APIVecFsCreateFolder, APIVecFsRetrievePathSimplifiedJson,
    APIVecFsRetrieveVectorSearchSimplifiedJson, MessageSchemaType,
};
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_node::llm_provider::execution::user_message_parser::ParsedUserMessage;
use shinkai_sqlite::SqliteManager;
use shinkai_vector_fs::vector_fs;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_fs::vector_fs::vector_fs_permissions::{ReadPermission, WritePermission};
use shinkai_vector_resources::data_tags::DataTag;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::file_parser::file_parser::ShinkaiFileParser;
{EmbeddingModelType, OllamaTextEmbeddingsInference};
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::source::{DistributionInfo, SourceFile, SourceFileMap, SourceFileType};
use shinkai_vector_resources::vector_resource::{simplified_fs_types::*, VRPack};
use shinkai_vector_resources::vector_resource::{
    BaseVectorResource, DocumentVectorResource, VRKai, VRPath, VRSourceReference, VectorResourceCore,
    VectorResourceSearch,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use crate::it::utils::node_test_api::{
    api_initial_registration_with_no_code_for_device, api_llm_provider_registration,
};
use crate::it::utils::shinkai_testing_framework::ShinkaiTestingFramework;
use crate::it::vector_fs_api_tests::generate_message_with_payload;

use super::utils;
use super::utils::db_handlers::setup_node_storage_path;
use super::utils::test_boilerplate::run_test_one_node_network;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);

    setup_node_storage_path();
}

fn default_test_profile() -> ShinkaiName {
    ShinkaiName::new("@@localhost.shinkai/profileName".to_string()).unwrap()
}

fn node_name() -> ShinkaiName {
    ShinkaiName::new("@@localhost.shinkai".to_string()).unwrap()
}

async fn setup_default_vector_fs(db: Arc<SqliteManager>) -> VectorFS {
    let generator = RemoteEmbeddingGenerator::new_default();
    let profile_list = vec![default_test_profile()];
    let supported_embedding_models = vec![EmbeddingModelType::OllamaTextEmbeddingsInference(
        OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M,
    )];

    VectorFS::new(generator, supported_embedding_models, profile_list, db, node_name())
        .await
        .unwrap()
}

pub async fn get_shinkai_intro_doc_async(
    generator: &RemoteEmbeddingGenerator,
    data_tags: &Vec<DataTag>,
) -> Result<(DocumentVectorResource, SourceFileMap), VRError> {
    // Initialize local PDF parser
    ShinkaiTestingFramework::initialize_pdfium().await;

    // Read the pdf from file into a buffer
    let source_file_name = "shinkai_intro.pdf";
    let buffer = std::fs::read(format!("../../files/{}", source_file_name)).map_err(|_| VRError::FailedPDFParsing)?;

    let desc = "An initial introduction to the Shinkai Network.";
    let resource = ShinkaiFileParser::process_file_into_resource(
        buffer.clone(),
        generator,
        "shinkai_intro.pdf".to_string(),
        Some(desc.to_string()),
        data_tags,
        500,
        DistributionInfo::new_empty(),
    )
    .await
    .unwrap();

    let file_type = SourceFileType::detect_file_type(source_file_name).unwrap();
    let source_file = SourceFile::new_standard_source_file(source_file_name.to_string(), file_type, buffer, None);
    let mut map = HashMap::new();
    map.insert(VRPath::root(), source_file);

    Ok((resource.as_document_resource_cloned().unwrap(), SourceFileMap::new(map)))
}

pub fn get_shinkai_intro_doc(generator: &RemoteEmbeddingGenerator, data_tags: &Vec<DataTag>) -> DocumentVectorResource {
    // Create a new Tokio runtime
    let rt = Runtime::new().unwrap();

    // Use block_on to run the async-based get_shinkai_intro_doc_async function
    let (resource, _) = rt.block_on(get_shinkai_intro_doc_async(generator, data_tags)).unwrap();

    resource
}

// // Test to be used to re-generate the VRKai/VRPack file whenever breaking changes take place.
// #[tokio::test]
// async fn test_gen_vrkai() {
//     setup();
//     let generator = RemoteEmbeddingGenerator::new_default();
//     let (doc_resource, source_file_map) = get_shinkai_intro_doc_async(&generator, &vec![])
//         .await
//         .expect("Failed to get shinkai intro doc");
//     let resource = BaseVectorResource::Document(doc_resource);
//     // With source file map
//     // let vrkai = VRKai::new(resource, Some(source_file_map), None);
//     // Without source file map
//     let vrkai = VRKai::new(resource, None);
//     let vrkai_bytes = vrkai.encode_as_bytes().expect("Failed to prepare VRKai bytes");
//     std::fs::write("../../files/shinkai_intro.vrkai", &vrkai_bytes).expect("Failed to write VRKai bytes to file");

//     let mut vrpack = VRPack::new_empty("shinkai-intro");
//     vrpack.insert_vrkai(&vrkai, VRPath::root());
//     let vrpack_bytes = vrpack.encode_as_bytes().expect("Failed to prepare VRPack bytes");
//     std::fs::write("../../files/shinkai_intro.vrpack", &vrpack_bytes).expect("Failed to write VRPack bytes to file");

//     // Read back and parse the VRKai file to verify it can be successfully decoded
//     let vrkai_bytes_read = std::fs::read("../../files/shinkai_intro.vrkai").expect("Failed to read VRKai file");
//     let parsed_vrkai = VRKai::from_bytes(&vrkai_bytes_read).expect("Failed to decode VRKai");
//     assert_eq!(
//         parsed_vrkai.encode_as_bytes().unwrap(),
//         vrkai_bytes,
//         "VRKai bytes mismatch after parsing"
//     );

//     // Read back and parse the VRPack file to verify it can be successfully decoded
//     let vrpack_bytes_read = std::fs::read("../../files/shinkai_intro.vrpack").expect("Failed to read VRPack file");
//     let parsed_vrpack = VRPack::from_bytes(&vrpack_bytes_read).expect("Failed to decode VRPack");
//     assert_eq!(
//         parsed_vrpack.encode_as_bytes().unwrap(),
//         vrpack_bytes,
//         "VRPack bytes mismatch after parsing"
//     );
// }

#[tokio::test]
async fn test_vrkai_vrpack_vector_search() {
    setup();
    let generator = RemoteEmbeddingGenerator::new_default();

    // Read VRKai from file as utf8 string
    let vrkai_str = std::fs::read_to_string("../../files/shinkai_intro.vrkai").expect("Failed to read VRKai from file");
    let vrkai = VRKai::from_base64(&vrkai_str).expect("Failed to decode VRKai from string");

    // Read VRPack from file as utf8 string
    let vrpack_bytes_read = std::fs::read("../../files/shinkai_intro.vrpack").expect("Failed to read VRPack file");
    let vrpack = VRPack::from_bytes(&vrpack_bytes_read).expect("Failed to decode VRPack");

    // Perform vector search on VRKai
    let query_string = "What is Shinkai?".to_string();
    let query_embedding = generator.generate_embedding_default(&query_string).await.unwrap();
    let vrkai_search_results = vrkai.vector_search(query_embedding, 100);

    // Perform vector search on VRPack
    let vrpack_search_results = vrpack
        .dynamic_deep_vector_search(query_string, 100, 100, generator, vec![])
        .await
        .unwrap();

    // Validate search results are equal
    assert_eq!(vrkai_search_results.len(), vrpack_search_results.len());
    for (vrkai_result, vrpack_result) in vrkai_search_results.iter().zip(vrpack_search_results.iter()) {
        assert_eq!(vrkai_result.retrieval_path, vrpack_result.retrieval_path);
    }
}

#[tokio::test]
async fn test_vector_fs_initializes_new_profile_automatically() {
    setup();
    let db = utils::db_handlers::setup_test_db();
    let db = Arc::new(db);
    let vector_fs = setup_default_vector_fs(db.clone()).await;

    let fs_internals = vector_fs.get_profile_fs_internals_cloned(&default_test_profile()).await;
    assert!(fs_internals.is_ok())
}

#[tokio::test]
async fn test_vector_fs_saving_reading() {
    setup();
    let db = utils::db_handlers::setup_test_db();
    let db = Arc::new(db);
    let generator = RemoteEmbeddingGenerator::new_default();
    let mut vector_fs = setup_default_vector_fs(db.clone()).await;

    let path = VRPath::new();
    let writer = vector_fs
        .new_writer(default_test_profile(), path.clone(), default_test_profile())
        .await
        .unwrap();
    let folder_name = "first_folder";
    vector_fs.create_new_folder(&writer, folder_name).await.unwrap();
    let writer = vector_fs
        .new_writer(
            default_test_profile(),
            path.push_cloned(folder_name.to_string()),
            default_test_profile(),
        )
        .await
        .unwrap();
    let folder_name_2 = "second_folder";
    vector_fs.create_new_folder(&writer, folder_name_2).await.unwrap();

    // Validate new folder path points to an entry at all (not empty), then specifically a folder, and finally not to an item.
    let folder_path = path.push_cloned(folder_name.to_string());
    assert!(vector_fs
        .validate_path_points_to_entry(folder_path.clone(), &writer.profile)
        .await
        .is_ok());
    assert!(vector_fs
        .validate_path_points_to_folder(folder_path.clone(), &writer.profile)
        .await
        .is_ok());
    assert!(vector_fs
        .validate_path_points_to_item(folder_path.clone(), &writer.profile)
        .await
        .is_err());

    // Create a Vector Resource and source file to be added into the VectorFS
    let (doc_resource, source_file_map) = get_shinkai_intro_doc_async(&generator, &vec![]).await.unwrap();
    let resource = BaseVectorResource::Document(doc_resource);
    let writer = vector_fs
        .new_writer(default_test_profile(), folder_path.clone(), default_test_profile())
        .await
        .unwrap();
    vector_fs
        .save_vector_resource_in_folder(&writer, resource.clone(), Some(source_file_map.clone()))
        .await
        .unwrap();

    // Validate new item path points to an entry at all (not empty), then specifically an item, and finally not to a folder.
    let item_path = folder_path.push_cloned(resource.as_trait_object().name().to_string());
    assert!(vector_fs
        .validate_path_points_to_entry(item_path.clone(), &writer.profile)
        .await
        .is_ok());
    assert!(vector_fs
        .validate_path_points_to_item(item_path.clone(), &writer.profile)
        .await
        .is_ok());
    assert!(vector_fs
        .validate_path_points_to_folder(item_path.clone(), &writer.profile)
        .await
        .is_err());

    let internals = vector_fs
        .get_profile_fs_internals_cloned(&default_test_profile())
        .await
        .unwrap();
    // internals.fs_core_resource.print_all_nodes_exhaustive(None, true, false);

    // Sets the permission to private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(default_test_profile(), item_path.clone(), default_test_profile())
        .await
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .await
        .unwrap();

    // Retrieve the Vector Resource & Source File Map from the db
    // Test both retrieve interfaces
    let reader = vector_fs
        .new_reader(default_test_profile(), item_path.clone(), default_test_profile())
        .await
        .unwrap();
    let ret_vrkai = vector_fs.retrieve_vrkai(&reader).await.unwrap();
    let (ret_resource, ret_source_file_map) = (ret_vrkai.resource, ret_vrkai.sfm);
    assert_eq!(ret_resource, resource);
    assert_eq!(ret_source_file_map, Some(source_file_map.clone()));

    println!("Keywords: {:?}", ret_resource.as_trait_object().keywords());
    assert!(ret_resource.as_trait_object().keywords().keyword_list.len() > 0);
    assert!(ret_resource.as_trait_object().keywords().keywords_embedding.is_some());

    let reader = vector_fs
        .new_reader(default_test_profile(), folder_path.clone(), default_test_profile())
        .await
        .unwrap();
    let ret_vrkai = vector_fs
        .retrieve_vrkai_in_folder(&reader, resource.as_trait_object().name().to_string())
        .await
        .unwrap();
    let (ret_resource, ret_source_file_map) = (ret_vrkai.resource, ret_vrkai.sfm);

    assert_eq!(ret_resource, resource);
    assert_eq!(ret_source_file_map, Some(source_file_map.clone()));

    //
    // Vector Search Tests
    //

    // First add a 2nd VR into the VecFS
    let generator = RemoteEmbeddingGenerator::new_default();
    let mut doc = DocumentVectorResource::new_empty(
        "3 Animal Facts",
        Some("A bunch of facts about animals and wildlife"),
        VRSourceReference::new_uri_ref("animalwildlife.com"),
        true,
    );
    doc.set_embedding_model_used(generator.model_type());
    doc.keywords_mut()
        .set_keywords(vec!["animal".to_string(), "wild life".to_string()]);
    doc.update_resource_embedding(&generator, None).await.unwrap();
    let fact1 = "Dogs are creatures with 4 legs that bark.";
    let fact1_embedding = generator.generate_embedding_default(fact1).await.unwrap();
    let fact2 = "Camels are slow animals with large humps.";
    let fact2_embedding = generator.generate_embedding_default(fact2).await.unwrap();
    let fact3 = "Seals swim in the ocean.";
    let fact3_embedding = generator.generate_embedding_default(fact3).await.unwrap();
    doc.append_text_node(fact1, None, fact1_embedding.clone(), &vec![])
        .unwrap();
    doc.append_text_node(fact2, None, fact2_embedding.clone(), &vec![])
        .unwrap();
    doc.append_text_node(fact3, None, fact3_embedding.clone(), &vec![])
        .unwrap();

    let writer = vector_fs
        .new_writer(default_test_profile(), folder_path.clone(), default_test_profile())
        .await
        .unwrap();
    let item = vector_fs
        .save_vector_resource_in_folder(
            &writer,
            BaseVectorResource::Document(doc),
            Some(source_file_map.clone()),
        )
        .await
        .unwrap();

    // Sets the permission to Private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(default_test_profile(), item.path.clone(), default_test_profile())
        .await
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .await
        .unwrap();

    // Searching for FSItems
    let reader = vector_fs
        .new_reader(default_test_profile(), VRPath::root(), default_test_profile())
        .await
        .unwrap();
    let query_string = "Who is building Shinkai?".to_string();
    println!("Query String: {}", query_string);
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string, &reader)
        .await
        .unwrap();
    let res = vector_fs
        .vector_search_fs_item(&reader, query_embedding, 100)
        .await
        .unwrap();
    assert_eq!(res[0].name(), "shinkai_intro");

    vector_fs.print_profile_vector_fs_resource(reader.profile.clone()).await;
    // Searching into the Vector Resources themselves in the VectorFS to acquire internal nodes
    let reader = vector_fs
        .new_reader(default_test_profile(), VRPath::root(), default_test_profile())
        .await
        .unwrap();
    let query_string = "Who is building Shinkai?".to_string();
    println!("Query String: {}", query_string);
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string.clone(), &reader)
        .await
        .unwrap();
    let res = vector_fs
        .deep_vector_search(&reader, query_string.clone(), 100, 100, vec![])
        .await
        .unwrap();
    assert_eq!(
        "Shinkai Network Manifesto (Early Preview)",
        res[0]
            .resource_retrieved_node
            .node
            .get_text_content()
            .unwrap()
            .to_string()
    );
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding, 1)
        .await
        .unwrap();
    assert_eq!("shinkai_intro", res[0].as_trait_object().name());

    // Animal facts search
    let query_string = "What do you know about camels?".to_string();
    println!("Query String: {}", query_string);
    let res = vector_fs
        .deep_vector_search(&reader, query_string.clone(), 100, 100, vec![])
        .await
        .unwrap();
    assert_eq!(
        "Camels are slow animals with large humps.",
        res[0]
            .resource_retrieved_node
            .node
            .get_text_content()
            .unwrap()
            .to_string()
    );

    // Vector Search W/Full VR Retrieval
    let query_string = "What are popular animals?".to_string();
    println!("Query String: {}", query_string);
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string, &reader)
        .await
        .unwrap();
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding, 100)
        .await
        .unwrap();
    assert_eq!("3 Animal Facts", res[0].as_trait_object().name());

    let query_string = "Shinkai intro pdf".to_string();
    println!("Query String: {}", query_string);
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string, &reader)
        .await
        .unwrap();
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding, 100)
        .await
        .unwrap();
    assert_eq!("shinkai_intro", res[0].as_trait_object().name());

    // Validate permissions checking in reader gen
    let invalid_requester =
        ShinkaiName::from_node_and_profile_names("alice".to_string(), "mainProfile".to_string()).unwrap();
    let reader = vector_fs
        .new_reader(invalid_requester.clone(), VRPath::root(), default_test_profile())
        .await;
    assert!(reader.is_err());

    // Validate permissions checking in Vector Search
    let writer = vector_fs
        .new_writer(default_test_profile(), VRPath::root(), default_test_profile())
        .await
        .unwrap();
    vector_fs
        .set_path_permission(&writer, ReadPermission::Whitelist, WritePermission::Private)
        .await
        .unwrap();
    vector_fs
        .set_whitelist_permission(
            &writer,
            invalid_requester.clone(),
            shinkai_vector_fs::vector_fs::vector_fs_permissions::WhitelistPermission::Read,
        )
        .await
        .unwrap();

    let reader = vector_fs
        .new_reader(invalid_requester.clone(), VRPath::root(), default_test_profile())
        .await
        .unwrap();
    let query_string = "Shinkai intro pdf".to_string();
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string, &reader)
        .await
        .unwrap();
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding.clone(), 100)
        .await
        .unwrap();
    assert_eq!(res.len(), 0);

    // Now give permission to first folder and see if results return the VRHeader in it
    let first_folder_path = VRPath::new().push_cloned(folder_name.to_string());
    let writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    vector_fs
        .set_path_permission(&writer, ReadPermission::Whitelist, WritePermission::Private)
        .await
        .unwrap();
    vector_fs
        .set_whitelist_permission(
            &writer,
            invalid_requester.clone(),
            shinkai_vector_fs::vector_fs::vector_fs_permissions::WhitelistPermission::Read,
        )
        .await
        .unwrap();

    {
        let internals = vector_fs
            .get_profile_fs_internals_cloned(&default_test_profile())
            .await
            .unwrap();

        println!("FS permissions: {:?}", internals.permissions_index.fs_permissions);
    }

    let reader = vector_fs
        .new_reader(
            invalid_requester.clone(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding.clone(), 100)
        .await
        .unwrap();
    assert!(res.len() == 0);
    let res = vector_fs
        .vector_search_vr_header(&reader, query_embedding.clone(), 100)
        .await
        .unwrap();
    assert!(res.len() > 0);

    // Now give permission to the item in the folder and see that the resource is returned
    let writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.push_cloned("shinkai_intro".to_string()),
            default_test_profile(),
        )
        .await
        .unwrap();
    vector_fs
        .set_path_permission(&writer, ReadPermission::Whitelist, WritePermission::Private)
        .await
        .unwrap();
    vector_fs
        .set_whitelist_permission(
            &writer,
            invalid_requester.clone(),
            vector_fs::vector_fs_permissions::WhitelistPermission::Read,
        )
        .await
        .unwrap();
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding.clone(), 100)
        .await
        .unwrap();
    assert!(!res.is_empty());
}

#[tokio::test]
async fn test_vector_fs_operations() {
    setup();
    let db = utils::db_handlers::setup_test_db();
    let db = Arc::new(db);
    let generator = RemoteEmbeddingGenerator::new_default();
    let mut vector_fs = setup_default_vector_fs(db.clone()).await;

    let writer = vector_fs
        .new_writer(default_test_profile(), VRPath::root(), default_test_profile())
        .await
        .unwrap();
    let folder_name = "first_folder";
    let first_folder_path = VRPath::root().push_cloned(folder_name.to_string());
    vector_fs.create_new_folder(&writer, folder_name).await.unwrap();

    // Sets the permission to Private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .await
        .unwrap();

    // Create a folder inside of first_folder
    let writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    let folder_name_2 = "second_folder";
    vector_fs.create_new_folder(&writer, folder_name_2).await.unwrap();
    let second_folder_path = first_folder_path.push_cloned(folder_name_2.to_string());

    // Sets the permission to Private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(
            default_test_profile(),
            second_folder_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .await
        .unwrap();

    // Create a Vector Resource and source file to be added into the VectorFS
    let (doc_resource, source_file_map) = get_shinkai_intro_doc_async(&generator, &vec![]).await.unwrap();
    let resource = BaseVectorResource::Document(doc_resource);
    let resource_name = resource.as_trait_object().name();
    let resource_ref_string = resource.as_trait_object().reference_string();
    let resource_merkle_root = resource.as_trait_object().get_merkle_root();
    let resource_node_count = resource.as_document_resource_cloned().unwrap().node_count();
    let writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    let first_folder_item = vector_fs
        .save_vector_resource_in_folder(&writer, resource.clone(), Some(source_file_map.clone()))
        .await
        .unwrap();

    //
    // Copy Tests
    //

    let writer = vector_fs
        .new_writer(default_test_profile(), VRPath::root(), default_test_profile())
        .await
        .unwrap();
    let new_root_folder_name = "new_root_folder".to_string();
    vector_fs
        .create_new_folder(&writer, &new_root_folder_name)
        .await
        .unwrap();
    let new_root_folder_path = VRPath::root().push_cloned(new_root_folder_name.clone());

    // Sets the permission to Private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(
            default_test_profile(),
            new_root_folder_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .await
        .unwrap();

    // Copy item from 1st folder into new root folder
    let orig_writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_item.path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    let dest_reader = orig_writer
        .new_reader_copied_data(new_root_folder_path.clone(), &mut vector_fs)
        .await
        .unwrap();
    vector_fs
        .copy_item(&orig_writer, new_root_folder_path.clone())
        .await
        .unwrap();
    let mut retrieved_vr = vector_fs
        .retrieve_vector_resource_in_folder(&dest_reader, resource_name.to_string())
        .await
        .unwrap();

    assert_eq!(resource_name, retrieved_vr.as_trait_object().name());
    assert_eq!(
        resource_node_count,
        retrieved_vr.as_document_resource().unwrap().node_count()
    );
    assert_eq!(resource_merkle_root, retrieved_vr.as_trait_object().get_merkle_root());
    assert_ne!(resource_ref_string, retrieved_vr.as_trait_object().reference_string());

    vector_fs.print_profile_vector_fs_resource(default_test_profile()).await;

    // Copy from new root folder to 2nd folder inside of first folder
    let root_folder_file_path = new_root_folder_path.push_cloned(resource_name.to_string());
    let orig_writer = vector_fs
        .new_writer(default_test_profile(), root_folder_file_path, default_test_profile())
        .await
        .unwrap();
    let dest_reader = orig_writer
        .new_reader_copied_data(second_folder_path.clone(), &mut vector_fs)
        .await
        .unwrap();
    vector_fs
        .copy_item(&orig_writer, second_folder_path.clone())
        .await
        .unwrap();
    let mut retrieved_vr = vector_fs
        .retrieve_vector_resource_in_folder(&dest_reader, resource_name.to_string())
        .await
        .unwrap();

    assert_eq!(resource_name, retrieved_vr.as_trait_object().name());
    assert_eq!(
        resource_node_count,
        retrieved_vr.as_document_resource().unwrap().node_count()
    );
    assert_eq!(resource_merkle_root, retrieved_vr.as_trait_object().get_merkle_root());
    assert_ne!(resource_ref_string, retrieved_vr.as_trait_object().reference_string());

    vector_fs.print_profile_vector_fs_resource(default_test_profile()).await;

    // Copy first folder as a whole into new root folder
    let new_root_folder_first_folder_path = new_root_folder_path.push_cloned(folder_name.to_string());
    let orig_writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    vector_fs
        .copy_folder(&orig_writer, new_root_folder_path.clone())
        .await
        .unwrap();
    let dest_reader = orig_writer
        .new_reader_copied_data(new_root_folder_first_folder_path.clone(), &mut vector_fs)
        .await
        .unwrap();
    let mut retrieved_vr = vector_fs
        .retrieve_vector_resource_in_folder(&dest_reader, resource_name.to_string())
        .await
        .unwrap();

    assert_eq!(resource_name, retrieved_vr.as_trait_object().name());
    assert_eq!(
        resource_node_count,
        retrieved_vr.as_document_resource().unwrap().node_count()
    );
    assert_eq!(resource_merkle_root, retrieved_vr.as_trait_object().get_merkle_root());
    assert_ne!(resource_ref_string, retrieved_vr.as_trait_object().reference_string());

    vector_fs.print_profile_vector_fs_resource(default_test_profile()).await;

    let node = vector_fs
        ._retrieve_core_resource_node_at_path(dest_reader.path.clone(), &dest_reader.profile)
        .await
        .unwrap();
    println!(
        "Folder keywords: {:?}",
        node.node
            .get_vector_resource_content()
            .unwrap()
            .as_trait_object()
            .keywords()
    );

    // Copying into a folder which does not exist fails
    let non_existent_folder_path = VRPath::root().push_cloned("non_existent_folder".to_string());
    let orig_writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    let copy_result = vector_fs
        .copy_folder(&orig_writer, non_existent_folder_path.clone())
        .await;
    assert!(copy_result.is_err());

    //
    // Move/Deletion Tests For Items
    //

    // Moving item from one folder to another means previous path is empty & file is in new location
    let item_to_move_path = first_folder_path.push_cloned(resource_name.to_string());
    let destination_folder_path = second_folder_path.clone();
    let new_location_path = destination_folder_path.push_cloned(resource_name.to_string());
    let orig_writer = vector_fs
        .new_writer(
            default_test_profile(),
            item_to_move_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();

    let dest_writer = vector_fs
        .new_writer(
            default_test_profile(),
            new_location_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();

    // Validate item deletion works
    vector_fs.delete_item(&dest_writer).await.unwrap();

    let new_location_check = vector_fs
        .validate_path_points_to_entry(new_location_path.clone(), &default_test_profile())
        .await
        .is_err();
    assert!(new_location_check, "The item should now not exist.");

    // Validate item moving works
    vector_fs
        .move_item(&orig_writer, destination_folder_path.clone())
        .await
        .unwrap();

    let orig_location_check = vector_fs
        .validate_path_points_to_entry(item_to_move_path.clone(), &default_test_profile())
        .await
        .is_err();
    assert!(
        orig_location_check,
        "The item should no longer exist in the original location."
    );

    let new_location_check = vector_fs
        .validate_path_points_to_entry(new_location_path.clone(), &default_test_profile())
        .await
        .is_ok();
    assert!(new_location_check, "The item should now exist in the new location.");

    //
    // Update VR description test
    //
    let writer = dest_writer.clone();
    let reader = dest_writer
        .new_reader_copied_data(dest_writer.path.clone(), &mut vector_fs)
        .await
        .unwrap();

    let retrieved_vr = vector_fs.retrieve_vector_resource(&reader).await.unwrap();
    let old_description = retrieved_vr.as_trait_object().description();

    let new_description = "New description".to_string();
    vector_fs
        .update_item_resource_description(&writer, new_description.to_string())
        .await
        .unwrap();

    let updated_retrieved_vr = vector_fs.retrieve_vector_resource(&reader).await.unwrap();

    assert_ne!(old_description, updated_retrieved_vr.as_trait_object().description());
    assert_eq!(
        new_description,
        updated_retrieved_vr
            .as_trait_object()
            .description()
            .unwrap()
            .to_string()
    );

    // VRPack creation & unpacking into VecFS tests
    //

    vector_fs.print_profile_vector_fs_resource(default_test_profile()).await;

    let reader = orig_writer
        .new_reader_copied_data(VRPath::root(), &mut vector_fs)
        .await
        .unwrap();
    let vrpack = vector_fs.retrieve_vrpack(&reader).await.unwrap();

    vrpack
        .resource
        .as_trait_object()
        .print_all_nodes_exhaustive(None, true, false);

    let unpacked_kais = vrpack.unpack_all_vrkais().unwrap();

    assert_eq!(unpacked_kais.len(), 4);

    // Now retrieve vrpack for non-root folder
    let reader = orig_writer
        .new_reader_copied_data(
            VRPath::root().push_cloned("new_root_folder".to_string()),
            &mut vector_fs,
        )
        .await
        .unwrap();

    println!("\n\n\nVectorFS:");
    vector_fs.print_profile_vector_fs_resource(default_test_profile()).await;

    let vrpack = vector_fs.retrieve_vrpack(&reader).await.unwrap();

    println!("\n\n\nVRPack:");
    vrpack.print_internal_structure(None);

    let unpacked_kais = vrpack.unpack_all_vrkais().unwrap();

    assert_eq!(unpacked_kais.len(), 3);

    // Now testing unpacking back into the VectorFS

    let unpack_path = VRPath::root().push_cloned("unpacked".to_string());
    assert!(vector_fs
        .validate_path_points_to_entry(unpack_path.clone(), &default_test_profile())
        .await
        .is_err());

    // Prepare a writer for the 'unpacked' folder
    let unpack_writer = vector_fs
        .new_writer(default_test_profile(), unpack_path.clone(), default_test_profile())
        .await
        .unwrap();

    // Unpack the VRPack into the 'unpacked' folder
    vector_fs
        .extract_vrpack_in_folder(&unpack_writer, vrpack.clone())
        .await
        .unwrap();

    // Verify the 'unpacked' folder now exists
    assert!(vector_fs
        .validate_path_points_to_folder(unpack_path.clone(), &unpack_writer.profile)
        .await
        .is_ok());

    let unpack_writer = vector_fs
        .new_writer(
            default_test_profile().clone(),
            unpack_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();
    let json = vector_fs.retrieve_fs_path_simplified_json(&reader).await.unwrap();
    let simplified_folder = SimplifiedFSEntry::from_json(&json).unwrap();

    assert_eq!(simplified_folder.clone().as_folder().unwrap().child_items.len(), 1);
    assert_eq!(simplified_folder.as_folder().unwrap().child_folders.len(), 1);

    vector_fs.print_profile_vector_fs_resource(default_test_profile()).await;

    // Compare original vrpack with new re-created vrpack

    let old_vrpack = vrpack.clone();
    let old_vrpack_contents = old_vrpack.unpack_all_vrkais().unwrap();

    let reader = orig_writer
        .new_reader_copied_data(
            ShinkaiPath::from_string("/unpacked/new_root_folder").unwrap(),
            &mut vector_fs,
        )
        .await
        .unwrap();
    let new_vrpack = vector_fs.retrieve_vrpack(&reader).await.unwrap();
    let new_vrpack_contents = new_vrpack.unpack_all_vrkais().unwrap();

    println!("\n\nOld VRPack:");
    old_vrpack.print_internal_structure(None);
    println!("\n\nNew VRPack:");
    new_vrpack.print_internal_structure(None);

    let mut old_vrpack_map = old_vrpack_contents
        .into_iter()
        .map(|(vrkai, path)| (path, vrkai))
        .collect::<HashMap<_, _>>();

    for (new_vrkai, new_path) in new_vrpack_contents {
        if let Some(old_vrkai) = old_vrpack_map.remove(&new_path) {
            assert_eq!(
                old_vrkai.resource.as_trait_object().reference_string(),
                new_vrkai.resource.as_trait_object().reference_string(),
                "Mismatch for path: {}",
                new_path
            );
        } else {
            panic!("New path not found in old VRPack contents: {}", new_path);
        }
    }

    assert!(
        old_vrpack_map.is_empty(),
        "Not all old VRPack contents were found in new: {:?}",
        old_vrpack_map.keys().collect::<Vec<_>>()
    );

    // Cleanup after vrpack tests
    let deletion_writer = unpack_writer
        .new_writer_copied_data(VRPath::root().push_cloned("unpacked".to_string()), &mut vector_fs)
        .await
        .unwrap();
    vector_fs.delete_folder(&deletion_writer).await.unwrap();

    //
    // Move/Deletion Tests for Folders
    //

    // Moving a folder from one location to another means the previous path is empty & the folder is in the new location
    let folder_name = "new_root_folder".to_string();
    let folder_to_move_path = VRPath::root().push_cloned(folder_name.to_string());
    let destination_folder_path = second_folder_path.clone();
    let new_folder_location_path = destination_folder_path.push_cloned(folder_name.to_string());

    let orig_folder_writer = vector_fs
        .new_writer(
            default_test_profile(),
            folder_to_move_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();

    // Validate folder moving works

    vector_fs
        .move_folder(&orig_folder_writer, destination_folder_path.clone())
        .await
        .unwrap();

    // vector_fs.print_profile_vector_fs_resource(default_test_profile());

    let orig_folder_location_check = vector_fs
        .validate_path_points_to_entry(folder_to_move_path.clone(), &default_test_profile())
        .await
        .is_err();
    assert!(
        orig_folder_location_check,
        "The folder should no longer exist in the original location."
    );

    let new_folder_location_check = vector_fs
        .validate_path_points_to_entry(new_folder_location_path.clone(), &default_test_profile())
        .await
        .is_ok();
    assert!(
        new_folder_location_check,
        "The folder should now exist in the new location."
    );

    // Validate folder deletion works
    let folder_to_delete_writer = vector_fs
        .new_writer(
            default_test_profile(),
            new_folder_location_path.clone(),
            default_test_profile(),
        )
        .await
        .unwrap();

    vector_fs.delete_folder(&folder_to_delete_writer).await.unwrap();

    let folder_deletion_check = vector_fs
        .validate_path_points_to_entry(new_folder_location_path.clone(), &default_test_profile())
        .await
        .is_err();
    assert!(folder_deletion_check, "The folder should now not exist.");

    //
    // Validate that for every folder/item, there is a matching path permission, and no more
    //
    let reader = orig_writer
        .new_reader_copied_data(VRPath::root(), &mut vector_fs)
        .await
        .unwrap();

    let fs_internals = vector_fs
        .get_profile_fs_internals_cloned(&default_test_profile())
        .await
        .unwrap();

    println!("\n\n\nVectorFS:");
    fs_internals
        .fs_core_resource
        .print_all_nodes_exhaustive(None, true, false);

    let all_read_perms = vector_fs
        .find_paths_with_read_permissions_as_hashmap(&reader, vec![ReadPermission::Public, ReadPermission::Private])
        .await
        .unwrap();
    let all_write_perms = vector_fs
        .find_paths_with_write_permissions_as_hashmap(&reader, vec![WritePermission::Private])
        .await
        .unwrap();
    let read_perms_count = all_read_perms.len();
    let write_perms_count = all_write_perms.len();

    let ret_nodes = fs_internals.fs_core_resource.retrieve_nodes_exhaustive_unordered(None);
    let all_internals_paths = ret_nodes.iter().map(|p| p.retrieval_path.clone());
    let paths_count = all_internals_paths.len();

    println!("All read read perms: {:?}", all_read_perms.keys());
    println!("All write write perms: {:?}", all_write_perms.keys());

    for path in all_internals_paths {
        println!("Path: {}", path);
        assert_eq!(all_read_perms.contains_key(&path), true);
        assert_eq!(all_write_perms.contains_key(&path), true);
    }
    for path in all_read_perms.keys() {
        assert_eq!(all_write_perms.contains_key(&path), true);
    }
    for path in all_write_perms.keys() {
        assert_eq!(all_read_perms.contains_key(&path), true);
    }
    assert_eq!(read_perms_count, paths_count);
    assert_eq!(write_perms_count, paths_count);

    //
    // Validate that after everything, in-memory state == fsdb state after reverting
    //
    let reader = orig_writer
        .new_reader_copied_data(VRPath::root(), &mut vector_fs)
        .await
        .unwrap();
    let writer = orig_writer
        .new_writer_copied_data(VRPath::root(), &mut vector_fs)
        .await
        .unwrap();
    let current_state = vector_fs.retrieve_fs_path_simplified_json(&reader).await.unwrap();
    vector_fs
        .revert_internals_to_last_db_save(&writer.profile, &writer.profile)
        .await
        .unwrap();
    let new_state = vector_fs.retrieve_fs_path_simplified_json(&reader).await.unwrap();

    assert_eq!(current_state, new_state);

    // Verify that

    //
    // Verify Simplified FSEntry types parse properly
    //
    let reader = orig_writer
        .new_reader_copied_data(VRPath::root(), &mut vector_fs)
        .await
        .unwrap();
    let root_json = vector_fs.retrieve_fs_path_simplified_json(&reader).await.unwrap();

    let simplified_root = SimplifiedFSEntry::from_json(&root_json);

    assert!(simplified_root.is_ok());

    let reader = orig_writer
        .new_reader_copied_data(
            ShinkaiPath::from_string("/first_folder/second_folder/").unwrap(),
            &mut vector_fs,
        )
        .await
        .unwrap();
    let json = vector_fs.retrieve_fs_path_simplified_json(&reader).await.unwrap();
    // println!("\n\n folder: {:?}", json);

    let simplified_folder = SimplifiedFSEntry::from_json(&json);
    assert!(simplified_folder.is_ok());

    let reader = orig_writer
        .new_reader_copied_data(
            ShinkaiPath::from_string("/first_folder/second_folder/shinkai_intro").unwrap(),
            &mut vector_fs,
        )
        .await
        .unwrap();
    let json = vector_fs.retrieve_fs_path_simplified_json(&reader).await.unwrap();
    // println!("\n\n item: {:?}", json);

    let simplified_folder = SimplifiedFSEntry::from_json(&json);
    assert!(simplified_folder.is_ok());
}

#[tokio::test]
async fn test_folder_empty_check_reuse() {
    setup();
    let db = utils::db_handlers::setup_test_db();
    let db = Arc::new(db);
    let generator = RemoteEmbeddingGenerator::new_default();
    let vector_fs = setup_default_vector_fs(db.clone()).await;

    // Create a new folder that will be checked for emptiness, then filled
    let folder_name = "test_folder";
    let folder_path = VRPath::root().push_cloned(folder_name.to_string());
    let writer = vector_fs
        .new_writer(default_test_profile(), VRPath::root(), default_test_profile())
        .await
        .unwrap();
    vector_fs.create_new_folder(&writer, folder_name).await.unwrap();

    // Check if the new folder is empty initially
    let reader = vector_fs
        .new_reader(default_test_profile(), folder_path.clone(), default_test_profile())
        .await
        .unwrap();
    assert!(
        vector_fs.is_folder_empty(&reader).await.unwrap(),
        "The folder should initially be empty."
    );

    // Add a document to the folder, making it non-empty
    let (doc_resource, source_file_map) = get_shinkai_intro_doc_async(&generator, &vec![]).await.unwrap();
    let resource = BaseVectorResource::Document(doc_resource);
    let writer = vector_fs
        .new_writer(default_test_profile(), folder_path.clone(), default_test_profile())
        .await
        .unwrap();
    vector_fs
        .save_vector_resource_in_folder(&writer, resource.clone(), Some(source_file_map.clone()))
        .await
        .unwrap();

    // Re-check if the folder is now non-empty
    let reader = vector_fs
        .new_reader(default_test_profile(), folder_path.clone(), default_test_profile())
        .await
        .unwrap();
    assert!(
        !vector_fs.is_folder_empty(&reader).await.unwrap(),
        "The folder should now be non-empty."
    );

    // Create a subfolder within the initial folder
    let subfolder_name = "subfolder1";
    let subfolder_path = folder_path.push_cloned(subfolder_name.to_string());
    vector_fs.create_new_folder(&writer, subfolder_name).await.unwrap();

    let writer = vector_fs
        .new_writer(default_test_profile(), subfolder_path.clone(), default_test_profile())
        .await
        .unwrap();

    let subfolder_name = "subfolder2";
    vector_fs.create_new_folder(&writer, subfolder_name).await.unwrap();

    // Check if the folder is still recognized as non-empty after adding a subfolder
    // This is to ensure the folder's non-empty status is consistent with its content state
    let reader = vector_fs
        .new_reader(default_test_profile(), subfolder_path, default_test_profile())
        .await
        .unwrap();
    assert!(
        !vector_fs.is_folder_empty(&reader).await.unwrap(),
        "The folder should still be non-empty after adding a subfolder."
    );
}

#[tokio::test]
async fn test_remove_code_blocks_with_parsed_user_message() {
    // Example strings containing code blocks
    let example1 = "Here is some text.\n```\nlet x = 10;\n```\nAnd here is more text.";
    let example2 = "Another example with a `single backtick`, and a code block:\n```\nfn main() {\n    println!(\"Hello, world!\");\n}\n```\nEnd of example.";
    let example3 = "Text before code block 1.\n```\nCode Block 1\n```\nText between code block 1 and 2.\n```\nCode Block 2\n```\nText between code block 2 and 3.\n```\nCode Block 3\n```\nText after code block 3.";

    // Create a parsed job task for each example
    let parsed_user_message1 = ParsedUserMessage::new(example1.to_string());
    let parsed_user_message2 = ParsedUserMessage::new(example2.to_string());
    let parsed_user_message3 = ParsedUserMessage::new(example3.to_string());

    // Extract only the code blocks from each parsed job task
    let code_blocks1 = parsed_user_message1.get_output_string_filtered(true, false);
    let code_blocks2 = parsed_user_message2.get_output_string_filtered(true, false);
    let code_blocks3 = parsed_user_message3.get_output_string_filtered(true, false);

    // Expected code blocks strings
    let expected_code_blocks1 = "```\nlet x = 10;\n```";
    let expected_code_blocks2 = "```\nfn main() {\n    println!(\"Hello, world!\");\n}\n```";
    let expected_code_blocks3 = "```\nCode Block 1\n```\n\n```\nCode Block 2\n```\n\n```\nCode Block 3\n```";

    // Assert that the extracted code blocks match the expected strings
    assert_eq!(code_blocks1, expected_code_blocks1);
    assert_eq!(code_blocks2, expected_code_blocks2);
    assert_eq!(code_blocks3, expected_code_blocks3);
}

// #[tokio::test]
// async fn test_parse_list_elements() {
//     // Example strings containing different types of lists
//     let example1 = "Here is some text.\n- Item 1\n- Item 2\nAnd here is more text.";
//     let example2 = "Another example text.\n* Item 1\n* Item 2\n* Item 3\nEnd of example.";
//     let example3 = "Text before numbered list.\n1. Item 1\n2. Item 2\n3. Item 3\nText after numbered list.";

//     // Create a parsed job task for each example
//     let parsed_user_message1 = ParsedUserMessage::new(example1.to_string());
//     let parsed_user_message2 = ParsedUserMessage::new(example2.to_string());
//     let parsed_user_message3 = ParsedUserMessage::new(example3.to_string());

//     // Assuming a method to count list elements in the parsed job task
//     let list_count1 = parsed_user_message1.get_elements_filtered(true, true, false).len();
//     let list_count2 = parsed_user_message2.get_elements_filtered(true, true, false).len();
//     let list_count3 = parsed_user_message3.get_elements_filtered(true, true, false).len();

//     // Expected number of list elements
//     let expected_list_count1 = 2;
//     let expected_list_count2 = 3;
//     let expected_list_count3 = 3;

//     // Assert that the counted list elements match the expected numbers
//     assert_eq!(
//         list_count1, expected_list_count1,
//         "List count in example1 does not match."
//     );
//     assert_eq!(
//         list_count2, expected_list_count2,
//         "List count in example2 does not match."
//     );
//     assert_eq!(
//         list_count3, expected_list_count3,
//         "List count in example3 does not match."
//     );

//     // Print each list element for visual inspection (assuming a method to iterate and print list elements)
//     println!("List elements in example1:");
//     parsed_user_message1.print_list_elements();
//     println!("List elements in example2:");
//     parsed_user_message2.print_list_elements();
//     println!("List elements in example3:");
//     parsed_user_message3.print_list_elements();
// }

#[test]
fn vector_search_multiple_embedding_models_test() {
    setup();
    std::env::set_var("WELCOME_MESSAGE", "false");

    let server = Server::new();

    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_agent = env.node1_llm_provider.clone();
            let node1_encryption_pk = env.node1_encryption_pk;
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let node1_abort_handler = env.node1_abort_handler;

            let node1_db_weak = Arc::downgrade(&env.node1_db);

            // For this test
            let symmetrical_sk = unsafe_deterministic_aes_encryption_key(0);

            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_initial_registration_with_no_code_for_device(
                    node1_commands_sender.clone(),
                    env.node1_profile_name.as_str(),
                    env.node1_identity_name.as_str(),
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name.as_str(),
                )
                .await;
            }

            {
                // Register an Agent
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let agent_name = ShinkaiName::new(
                    format!(
                        "{}/{}/agent/{}",
                        node1_identity_name.clone(),
                        node1_profile_name.clone(),
                        node1_agent.clone()
                    )
                    .to_string(),
                )
                .unwrap();

                let ollama = Ollama {
                    model_type: "mixtral:8x7b-instruct-v0.1-q4_1".to_string(),
                };

                let agent = SerializedLLMProvider {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    api_key: Some("".to_string()),
                    external_url: Some(server.url()),
                    model: LLMProviderInterface::Ollama(ollama),
                };
                api_llm_provider_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    agent,
                )
                .await;
            }
            // Send message (APICreateFilesInboxWithSymmetricKey) from Device subidentity to Node 1
            {
                eprintln!("\n\n### Sending message (APICreateFilesInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

                let message_content = aes_encryption_key_to_string(symmetrical_sk);
                let msg = ShinkaiMessageBuilder::create_files_inbox_with_sym_key(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    "job::test::false".to_string(),
                    message_content.clone(),
                    node1_profile_name.to_string(),
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string(),
                )
                .unwrap();

                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res: res_sender })
                    .await
                    .unwrap();
                let _ = res_receiver.recv().await.unwrap().expect("Failed to receive messages");
            }
            {
                // Update supported embedding models
                let payload = [
                    OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M.to_string(),
                    OllamaTextEmbeddingsInference::JinaEmbeddingsV2BaseEs.to_string(),
                ];

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::UpdateSupportedEmbeddingModels,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIUpdateSupportedEmbeddingModels { msg, res: res_sender })
                    .await
                    .unwrap();

                // Receive the response
                let _ = res_receiver.recv().await.unwrap().expect("Failed to receive response");
            }
            {
                // Initialize local PDF parser
                ShinkaiTestingFramework::initialize_pdfium().await;

                // Create Folder
                let payload = APIVecFsCreateFolder {
                    path: "/".to_string(),
                    folder_name: "test_folder".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsCreateFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSCreateFolder { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp: {:?}", resp);
            }
            {
                // Upload .vrkai file with jina es embeddings to inbox
                // Prepare the file to be read
                let filename = "../../files/hispania_jina_es.vrkai";
                let file_path = Path::new(filename);

                // Read the file into a buffer
                let file_data = std::fs::read(file_path).map_err(|_| VRError::FailedPDFParsing).unwrap();

                // Encrypt the file using Aes256Gcm
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));
                let nonce = GenericArray::from_slice(&[0u8; 12]);
                let nonce_slice = nonce.as_slice();
                let nonce_str = aes_nonce_to_hex_string(nonce_slice);
                let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
                        filename: filename.to_string(),
                        file: ciphertext,
                        public_key: hash_of_aes_encryption_key_hex(symmetrical_sk),
                        encrypted_nonce: nonce_str,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                // Receive the response
                let _ = res_receiver.recv().await.unwrap().expect("Failed to receive response");
            }
            {
                // Convert File and Save to Folder
                let payload = APIConvertFilesAndSaveToFolder {
                    path: "/test_folder".to_string(),
                    file_inbox: hash_of_aes_encryption_key_hex(symmetrical_sk),
                    file_datetime: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::ConvertFilesAndSaveToFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIConvertFilesAndSaveToFolder { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp: {:?}", resp);
            }
            {
                // Recover file from path using APIVecFSRetrievePathSimplifiedJson
                let payload = APIVecFsRetrievePathSimplifiedJson {
                    path: "/test_folder/hispania".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                // eprintln!("resp for current file system files: {}", resp);

                // Assuming `resp` is now a serde_json::Value
                let resp_json = serde_json::to_string(&resp).expect("Failed to convert response to string");
                // eprintln!("resp for current file system files: {}", resp_json);

                // TODO: convert to json and then compare
                let expected_path = "/test_folder/hispania";
                assert!(
                    resp_json.contains(expected_path),
                    "Response does not contain the expected file path: {}",
                    expected_path
                );
            }
            {
                // Upload .vrkai file to inbox
                // Prepare the file to be read
                let filename = "../../files/short_story.md";
                let file_path = Path::new(filename);

                // Read the file into a buffer
                let file_data = std::fs::read(file_path).map_err(|_| VRError::FailedPDFParsing).unwrap();

                // Encrypt the file using Aes256Gcm
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));
                let nonce = GenericArray::from_slice(&[0u8; 12]);
                let nonce_slice = nonce.as_slice();
                let nonce_str = aes_nonce_to_hex_string(nonce_slice);
                let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
                        filename: filename.to_string(),
                        file: ciphertext,
                        public_key: hash_of_aes_encryption_key_hex(symmetrical_sk),
                        encrypted_nonce: nonce_str,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                // Receive the response
                let _ = res_receiver.recv().await.unwrap().expect("Failed to receive response");
            }
            {
                // Convert File and Save to Folder
                let payload = APIConvertFilesAndSaveToFolder {
                    path: "/test_folder".to_string(),
                    file_inbox: hash_of_aes_encryption_key_hex(symmetrical_sk),
                    file_datetime: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::ConvertFilesAndSaveToFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIConvertFilesAndSaveToFolder { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp: {:?}", resp);
            }
            {
                // Recover file from path using APIVecFSRetrievePathSimplifiedJson
                let payload = APIVecFsRetrievePathSimplifiedJson {
                    path: "/test_folder/short_story".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                // eprintln!("resp for current file system files: {}", resp);

                // Assuming `resp` is now a serde_json::Value
                let resp_json = serde_json::to_string(&resp).expect("Failed to convert response to string");
                // eprintln!("resp for current file system files: {}", resp_json);

                // TODO: convert to json and then compare
                let expected_path = "/test_folder/short_story";
                assert!(
                    resp_json.contains(expected_path),
                    "Response does not contain the expected file path: {}",
                    expected_path
                );
            }
            {
                // Do deep search
                let payload = APIVecFsRetrieveVectorSearchSimplifiedJson {
                    search: "Dónde estaba ubicada la capital principal?".to_string(),
                    path: None,
                    max_results: Some(10),
                    max_files_to_scan: Some(100),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrieveVectorSearchSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSRetrieveVectorSearchSimplifiedJson { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                for r in &resp {
                    eprintln!("\n\nSearch result: {:?}", r);
                }

                assert!(!resp.is_empty(), "Response is empty.");
                assert!(&resp
                    .iter()
                    .any(|r| r.0.contains("principal capital estaba situada en Qart Hadasht")));
            }
            node1_abort_handler.abort();
        })
    });
}
