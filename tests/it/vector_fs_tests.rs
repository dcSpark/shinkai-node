use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_node::agent::file_parsing::ParsingHelper;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::vector_fs::vector_fs_internals::VectorFSInternals;
use shinkai_node::vector_fs::vector_fs_permissions::{ReadPermission, WritePermission};
use shinkai_node::vector_fs::vector_fs_reader::VFSReader;
use shinkai_node::vector_fs::vector_fs_writer::VFSWriter;
use shinkai_node::vector_fs::{db::fs_db::VectorFSDB, vector_fs::VectorFS, vector_fs_error::VectorFSError};
use shinkai_vector_resources::data_tags::DataTag;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::model_type::{EmbeddingModelType, TextEmbeddingsInference};
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::source::{
    DistributionOrigin, SourceFile, SourceFileMap, SourceFileType, SourceReference,
};
use shinkai_vector_resources::unstructured::unstructured_api::UnstructuredAPI;
use shinkai_vector_resources::vector_resource::{
    BaseVectorResource, DocumentVectorResource, VRKai, VRPath, VRSource, VectorResource, VectorResourceCore,
    VectorResourceSearch,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tokio::runtime::Runtime;

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

fn setup_default_vector_fs() -> VectorFS {
    let generator = RemoteEmbeddingGenerator::new_default();
    let fs_db_path = format!("db_tests/{}", "vector_fs");
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

pub async fn get_shinkai_intro_doc_async(
    generator: &RemoteEmbeddingGenerator,
    data_tags: &Vec<DataTag>,
) -> Result<(DocumentVectorResource, SourceFileMap), VRError> {
    // Read the pdf from file into a buffer
    let source_file_name = "shinkai_intro.pdf";
    let buffer = std::fs::read(format!("files/{}", source_file_name.clone())).map_err(|_| VRError::FailedPDFParsing)?;

    let desc = "An initial introduction to the Shinkai Network.";
    let resource = ParsingHelper::parse_file_into_resource(
        buffer.clone(),
        generator,
        "shinkai_intro.pdf".to_string(),
        Some(desc.to_string()),
        data_tags,
        500,
        UnstructuredAPI::new_default(),
    )
    .await
    .unwrap();

    let file_type = SourceFileType::detect_file_type(&source_file_name).unwrap();
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

// // Test to be used to used once to re-generate the VRKai file whenever breaking changes take place.
// #[tokio::test]
// async fn test_gen_vrkai() {
//     setup();
//     let generator = RemoteEmbeddingGenerator::new_default();
//     let (doc_resource, source_file_map) = get_shinkai_intro_doc_async(&generator, &vec![]).await.unwrap();
//     let resource = BaseVectorResource::Document(doc_resource);
//     // With source file map
//     // let vrkai = VRKai::from_base_vector_resource(resource, Some(source_file_map), None);
//     // Without source file map
//     let vrkai = VRKai::from_base_vector_resource(resource, None, None);
//     let vrkai_bytes = vrkai.prepare_as_bytes().expect("Failed to prepare VRKai bytes");
//     std::fs::write("files/shinkai_intro.vrkai", vrkai_bytes).expect("Failed to write VRKai bytes to file");
// }

#[tokio::test]
async fn test_vector_fs_initializes_new_profile_automatically() {
    setup();
    let generator = RemoteEmbeddingGenerator::new_default();
    let mut vector_fs = setup_default_vector_fs();

    let fs_internals = vector_fs.get_profile_fs_internals(&default_test_profile());
    assert!(fs_internals.is_ok())
}

#[tokio::test]
async fn test_vector_fs_saving_reading() {
    setup();
    let generator = RemoteEmbeddingGenerator::new_default();
    let mut vector_fs = setup_default_vector_fs();

    let path = VRPath::new();
    let writer = vector_fs
        .new_writer(default_test_profile(), path.clone(), default_test_profile())
        .unwrap();
    let folder_name = "first_folder";
    vector_fs.create_new_folder(&writer, folder_name.clone()).unwrap();
    let writer = vector_fs
        .new_writer(
            default_test_profile(),
            path.push_cloned(folder_name.to_string()),
            default_test_profile(),
        )
        .unwrap();
    let folder_name_2 = "second_folder";
    vector_fs.create_new_folder(&writer, folder_name_2).unwrap();

    // Validate new folder path points to an entry at all (not empty), then specifically a folder, and finally not to an item.
    let folder_path = path.push_cloned(folder_name.to_string());
    assert!(vector_fs
        .validate_path_points_to_entry(folder_path.clone(), &writer.profile)
        .is_ok());
    assert!(vector_fs
        .validate_path_points_to_folder(folder_path.clone(), &writer.profile)
        .is_ok());
    assert!(vector_fs
        .validate_path_points_to_item(folder_path.clone(), &writer.profile)
        .is_err());

    // Create a Vector Resource and source file to be added into the VectorFS
    let (doc_resource, source_file_map) = get_shinkai_intro_doc_async(&generator, &vec![]).await.unwrap();
    let resource = BaseVectorResource::Document(doc_resource);
    let writer = vector_fs
        .new_writer(default_test_profile(), folder_path.clone(), default_test_profile())
        .unwrap();
    vector_fs
        .save_vector_resource_in_folder(&writer, resource.clone(), Some(source_file_map.clone()), None)
        .unwrap();

    // Validate new item path points to an entry at all (not empty), then specifically an item, and finally not to a folder.
    let item_path = folder_path.push_cloned(resource.as_trait_object().name().to_string());
    assert!(vector_fs
        .validate_path_points_to_entry(item_path.clone(), &writer.profile)
        .is_ok());
    assert!(vector_fs
        .validate_path_points_to_item(item_path.clone(), &writer.profile)
        .is_ok());
    assert!(vector_fs
        .validate_path_points_to_folder(item_path.clone(), &writer.profile)
        .is_err());

    let internals = vector_fs
        .get_profile_fs_internals_read_only(&default_test_profile())
        .unwrap();
    internals.fs_core_resource.print_all_nodes_exhaustive(None, true, false);

    // Sets the permission to private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(default_test_profile(), item_path.clone(), default_test_profile())
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .unwrap();

    /// Retrieve the Vector Resource & Source File Map from the db
    // Test both retrieve interfaces
    let reader = vector_fs
        .new_reader(default_test_profile(), item_path.clone(), default_test_profile())
        .unwrap();
    let ret_vrkai = vector_fs.retrieve_vrkai(&reader).unwrap();
    let (ret_resource, ret_source_file_map) = (ret_vrkai.resource, ret_vrkai.sfm);
    assert_eq!(ret_resource, resource);
    assert_eq!(ret_source_file_map, Some(source_file_map.clone()));

    println!("Keywords: {:?}", ret_resource.as_trait_object().keywords());
    assert!(ret_resource.as_trait_object().keywords().keyword_list.len() > 0);
    assert!(ret_resource.as_trait_object().keywords().keywords_embedding.is_some());

    let reader = vector_fs
        .new_reader(default_test_profile(), folder_path.clone(), default_test_profile())
        .unwrap();
    let ret_vrkai = vector_fs
        .retrieve_vrkai_in_folder(&reader, resource.as_trait_object().name().to_string())
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
        VRSource::new_uri_ref("animalwildlife.com", None),
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
    doc.append_text_node(fact1.clone(), None, fact1_embedding.clone(), &vec![])
        .unwrap();
    doc.append_text_node(fact2.clone(), None, fact2_embedding.clone(), &vec![])
        .unwrap();
    doc.append_text_node(fact3.clone(), None, fact3_embedding.clone(), &vec![])
        .unwrap();

    let writer = vector_fs
        .new_writer(default_test_profile(), folder_path.clone(), default_test_profile())
        .unwrap();
    let item = vector_fs
        .save_vector_resource_in_folder(
            &writer,
            BaseVectorResource::Document(doc),
            Some(source_file_map.clone()),
            None,
        )
        .unwrap();

    // Sets the permission to Private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(default_test_profile(), item.path.clone(), default_test_profile())
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .unwrap();

    // Searching for FSItems
    let reader = vector_fs
        .new_reader(default_test_profile(), VRPath::root(), default_test_profile())
        .unwrap();
    let query_string = "Who is building Shinkai?".to_string();
    println!("Query String: {}", query_string);
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string, &reader)
        .await
        .unwrap();
    let res = vector_fs.vector_search_fs_item(&reader, query_embedding, 100).unwrap();
    assert_eq!(res[0].name(), "shinkai_intro");

    vector_fs.print_profile_vector_fs_resource(reader.profile.clone());
    // Searching into the Vector Resources themselves in the VectorFS to acquire internal nodes
    let reader = vector_fs
        .new_reader(default_test_profile(), VRPath::root(), default_test_profile())
        .unwrap();
    let query_string = "Who is building Shinkai?".to_string();
    println!("Query String: {}", query_string);
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string.clone(), &reader)
        .await
        .unwrap();
    let res = vector_fs
        .deep_vector_search(&reader, query_string.clone(), 100, 100)
        .await
        .unwrap();
    assert_eq!(
        "Shinkai Network Manifesto (Early Preview) Robert Kornacki rob@shinkai.com Nicolas Arqueros",
        res[0]
            .resource_retrieved_node
            .node
            .get_text_content()
            .unwrap()
            .to_string()
    );
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding, 1)
        .unwrap();
    assert_eq!("shinkai_intro", res[0].as_trait_object().name());

    // Animal facts search
    let query_string = "What do you know about camels?".to_string();
    println!("Query String: {}", query_string);
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string.clone(), &reader)
        .await
        .unwrap();
    let res = vector_fs
        .deep_vector_search(&reader, query_string.clone(), 100, 100)
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
        .unwrap();
    assert_eq!("shinkai_intro", res[0].as_trait_object().name());

    // Validate permissions checking in reader gen
    let invalid_requester = ShinkaiName::from_node_and_profile("alice".to_string(), "mainProfile".to_string()).unwrap();
    let reader = vector_fs.new_reader(invalid_requester.clone(), VRPath::root(), default_test_profile());
    assert!(reader.is_err());

    // Validate permissions checking in Vector Search
    let writer = vector_fs
        .new_writer(default_test_profile(), VRPath::root(), default_test_profile())
        .unwrap();
    vector_fs
        .set_path_permission(&writer, ReadPermission::Whitelist, WritePermission::Private)
        .unwrap();
    vector_fs
        .set_whitelist_permission(
            &writer,
            invalid_requester.clone(),
            shinkai_node::vector_fs::vector_fs_permissions::WhitelistPermission::Read,
        )
        .unwrap();

    let reader = vector_fs
        .new_reader(invalid_requester.clone(), VRPath::root(), default_test_profile())
        .unwrap();
    let query_string = "Shinkai intro pdf".to_string();
    let query_embedding = vector_fs
        .generate_query_embedding_using_reader(query_string, &reader)
        .await
        .unwrap();
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding.clone(), 100)
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
        .unwrap();
    vector_fs
        .set_path_permission(&writer, ReadPermission::Whitelist, WritePermission::Private)
        .unwrap();
    vector_fs
        .set_whitelist_permission(
            &writer,
            invalid_requester.clone(),
            shinkai_node::vector_fs::vector_fs_permissions::WhitelistPermission::Read,
        )
        .unwrap();

    {
        let internals = vector_fs
            .get_profile_fs_internals_read_only(&default_test_profile())
            .unwrap();

        println!("FS permissions: {:?}", internals.permissions_index.fs_permissions);
    }

    let reader = vector_fs
        .new_reader(
            invalid_requester.clone(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .unwrap();
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding.clone(), 100)
        .unwrap();
    assert!(res.len() == 0);
    let res = vector_fs
        .vector_search_vr_header(&reader, query_embedding.clone(), 100)
        .unwrap();
    assert!(res.len() > 0);

    // Now give permission to the item in the folder and see that the resource is returned
    let writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.push_cloned("shinkai_intro".to_string()),
            default_test_profile(),
        )
        .unwrap();
    vector_fs
        .set_path_permission(&writer, ReadPermission::Whitelist, WritePermission::Private)
        .unwrap();
    vector_fs
        .set_whitelist_permission(
            &writer,
            invalid_requester.clone(),
            shinkai_node::vector_fs::vector_fs_permissions::WhitelistPermission::Read,
        )
        .unwrap();
    let res = vector_fs
        .vector_search_vector_resource(&reader, query_embedding.clone(), 100)
        .unwrap();
    assert!(res.len() > 0);
}

#[tokio::test]
async fn test_vector_fs_operations() {
    setup();
    let generator = RemoteEmbeddingGenerator::new_default();
    let mut vector_fs = setup_default_vector_fs();

    let writer = vector_fs
        .new_writer(default_test_profile(), VRPath::root(), default_test_profile())
        .unwrap();
    let folder_name = "first_folder";
    let first_folder_path = VRPath::root().push_cloned(folder_name.to_string());
    vector_fs.create_new_folder(&writer, folder_name.clone()).unwrap();

    // Sets the permission to Private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .unwrap();

    // Create a folder inside of first_folder
    let writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .unwrap();
    let folder_name_2 = "second_folder";
    vector_fs.create_new_folder(&writer, folder_name_2).unwrap();
    let second_folder_path = first_folder_path.push_cloned(folder_name_2.to_string());

    // Sets the permission to Private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(
            default_test_profile(),
            second_folder_path.clone(),
            default_test_profile(),
        )
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .unwrap();

    // Create a Vector Resource and source file to be added into the VectorFS
    let (doc_resource, source_file_map) = get_shinkai_intro_doc_async(&generator, &vec![]).await.unwrap();
    let mut resource = BaseVectorResource::Document(doc_resource);
    let resource_name = resource.as_trait_object().name().clone();
    let resource_ref_string = resource.as_trait_object().reference_string();
    let resource_merkle_root = resource.as_trait_object().get_merkle_root();
    let resource_node_count = resource.as_document_resource_cloned().unwrap().node_count().clone();
    let writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .unwrap();
    let first_folder_item = vector_fs
        .save_vector_resource_in_folder(&writer, resource.clone(), Some(source_file_map.clone()), None)
        .unwrap();

    //
    // Copy Tests
    //

    let writer = vector_fs
        .new_writer(default_test_profile(), VRPath::root(), default_test_profile())
        .unwrap();
    let new_root_folder_name = "new_root_folder".to_string();
    vector_fs.create_new_folder(&writer, &new_root_folder_name).unwrap();
    let new_root_folder_path = VRPath::root().push_cloned(new_root_folder_name.clone());

    // Sets the permission to Private from default Whitelist (for later test cases)
    let perm_writer = vector_fs
        .new_writer(
            default_test_profile(),
            new_root_folder_path.clone(),
            default_test_profile(),
        )
        .unwrap();
    vector_fs
        .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
        .unwrap();

    // Copy item from 1st folder into new root folder
    let orig_writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_item.path.clone(),
            default_test_profile(),
        )
        .unwrap();
    let dest_reader = orig_writer
        .new_reader_copied_data(new_root_folder_path.clone(), &mut vector_fs)
        .unwrap();
    vector_fs.copy_item(&orig_writer, new_root_folder_path.clone()).unwrap();
    let mut retrieved_vr = vector_fs
        .retrieve_vector_resource_in_folder(&dest_reader, resource_name.to_string())
        .unwrap();

    assert_eq!(resource_name, retrieved_vr.as_trait_object().name());
    assert_eq!(
        resource_node_count,
        retrieved_vr.as_document_resource().unwrap().node_count()
    );
    assert_eq!(resource_merkle_root, retrieved_vr.as_trait_object().get_merkle_root());
    assert_ne!(resource_ref_string, retrieved_vr.as_trait_object().reference_string());

    vector_fs.print_profile_vector_fs_resource(default_test_profile());

    // Copy from new root folder to 2nd folder inside of first folder
    let root_folder_file_path = new_root_folder_path.push_cloned(resource_name.to_string());
    let orig_writer = vector_fs
        .new_writer(default_test_profile(), root_folder_file_path, default_test_profile())
        .unwrap();
    let dest_reader = orig_writer
        .new_reader_copied_data(second_folder_path.clone(), &mut vector_fs)
        .unwrap();
    vector_fs.copy_item(&orig_writer, second_folder_path.clone()).unwrap();
    let mut retrieved_vr = vector_fs
        .retrieve_vector_resource_in_folder(&dest_reader, resource_name.to_string())
        .unwrap();

    assert_eq!(resource_name, retrieved_vr.as_trait_object().name());
    assert_eq!(
        resource_node_count,
        retrieved_vr.as_document_resource().unwrap().node_count()
    );
    assert_eq!(resource_merkle_root, retrieved_vr.as_trait_object().get_merkle_root());
    assert_ne!(resource_ref_string, retrieved_vr.as_trait_object().reference_string());

    vector_fs.print_profile_vector_fs_resource(default_test_profile());

    // Copy first folder as a whole into new root folder
    let new_root_folder_first_folder_path = new_root_folder_path.push_cloned(folder_name.to_string());
    let orig_writer = vector_fs
        .new_writer(
            default_test_profile(),
            first_folder_path.clone(),
            default_test_profile(),
        )
        .unwrap();
    let dest_reader = orig_writer
        .new_reader_copied_data(new_root_folder_first_folder_path.clone(), &mut vector_fs)
        .unwrap();
    vector_fs
        .copy_folder(&orig_writer, new_root_folder_path.clone())
        .unwrap();
    let mut retrieved_vr = vector_fs
        .retrieve_vector_resource_in_folder(&dest_reader, resource_name.to_string())
        .unwrap();

    assert_eq!(resource_name, retrieved_vr.as_trait_object().name());
    assert_eq!(
        resource_node_count,
        retrieved_vr.as_document_resource().unwrap().node_count()
    );
    assert_eq!(resource_merkle_root, retrieved_vr.as_trait_object().get_merkle_root());
    assert_ne!(resource_ref_string, retrieved_vr.as_trait_object().reference_string());

    vector_fs.print_profile_vector_fs_resource(default_test_profile());

    let node = vector_fs
        ._retrieve_core_resource_node_at_path(dest_reader.path.clone(), &dest_reader.profile)
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
        .unwrap();
    let copy_result = vector_fs.copy_folder(&orig_writer, non_existent_folder_path.clone());
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
        .unwrap();

    let dest_writer = vector_fs
        .new_writer(
            default_test_profile(),
            new_location_path.clone(),
            default_test_profile(),
        )
        .unwrap();

    // Validate item deletion works
    vector_fs.delete_item(&dest_writer).unwrap();

    let new_location_check = vector_fs
        .validate_path_points_to_entry(new_location_path.clone(), &default_test_profile())
        .is_err();
    assert!(new_location_check, "The item should now not exist.");

    // Validate item moving works
    vector_fs
        .move_item(&orig_writer, destination_folder_path.clone())
        .unwrap();

    let orig_location_check = vector_fs
        .validate_path_points_to_entry(item_to_move_path.clone(), &default_test_profile())
        .is_err();
    assert!(
        orig_location_check,
        "The item should no longer exist in the original location."
    );

    let new_location_check = vector_fs
        .validate_path_points_to_entry(new_location_path.clone(), &default_test_profile())
        .is_ok();
    assert!(new_location_check, "The item should now exist in the new location.");

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
        .unwrap();

    // Validate folder moving works

    vector_fs
        .move_folder(&orig_folder_writer, destination_folder_path.clone())
        .unwrap();

    vector_fs.print_profile_vector_fs_resource(default_test_profile());

    let orig_folder_location_check = vector_fs
        .validate_path_points_to_entry(folder_to_move_path.clone(), &default_test_profile())
        .is_err();
    assert!(
        orig_folder_location_check,
        "The folder should no longer exist in the original location."
    );

    let new_folder_location_check = vector_fs
        .validate_path_points_to_entry(new_folder_location_path.clone(), &default_test_profile())
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
        .unwrap();

    vector_fs.delete_folder(&folder_to_delete_writer).unwrap();

    let folder_deletion_check = vector_fs
        .validate_path_points_to_entry(new_folder_location_path.clone(), &default_test_profile())
        .is_err();
    assert!(folder_deletion_check, "The folder should now not exist.");

    //
    // Validate that after everything, in-memory state == fsdb state
    //
    let reader = orig_writer
        .new_reader_copied_data(VRPath::root(), &mut vector_fs)
        .unwrap();
    let writer = orig_writer
        .new_writer_copied_data(VRPath::root(), &mut vector_fs)
        .unwrap();
    let current_state = vector_fs.retrieve_fs_path_simplified_json(&reader).unwrap();
    vector_fs
        .revert_internals_to_last_db_save(&writer.profile, &writer.profile)
        .unwrap();
    let new_state = vector_fs.retrieve_fs_path_simplified_json(&reader).unwrap();

    assert_eq!(current_state, new_state);
}
