#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;
    use shinkai_fs_mirror::{
        shinkai::api_schemas::{DistributionInfo, FileInfo},
        synchronizer::FilesystemSynchronizer,
    };

    #[tokio::test]
    async fn test_extract_paths_and_hashes() {
        let input_json = json!({
            "child_folders": [
                {
                    "child_folders": [],
                    "child_items": [
                        {
                            "created_datetime": "2024-04-17T05:09:42.777742Z",
                            "distribution_info": {
                                "datetime": "2023-02-10T00:00:00Z",
                                "origin": null
                            },
                            "last_written_datetime": "2024-04-17T05:09:43.284776Z",
                            "merkle_hash": "1f35e80e0419746457b32a2a0a21b30f89a1674a05ed087971bb02e38d17211a",
                            "name": "how_satellite_images_reveal_whats_happening_in_ukraine",
                            "path": "/youtube/RealLifeLore/2023-02-10/how_satellite_images_reveal_whats_happening_in_ukraine",
                            "source_file_map_last_saved_datetime": "2024-04-17T05:09:43.286848Z",
                        }
                    ],
                    "created_datetime": "2024-04-17T05:09:09.056716Z",
                    "last_modified_datetime": "2024-04-17T05:09:43.286848Z",
                    "last_read_datetime": "2024-04-17T05:09:09.056714Z",
                }
            ]
        });

        let expected_output = HashMap::from([(
            "/youtube/RealLifeLore/2023-02-10/how_satellite_images_reveal_whats_happening_in_ukraine".to_string(),
            FileInfo {
                merkle_hash: "1f35e80e0419746457b32a2a0a21b30f89a1674a05ed087971bb02e38d17211a".to_string(),
                name: "how_satellite_images_reveal_whats_happening_in_ukraine".to_string(),
                source_file_map_last_saved_datetime: "2024-04-17T05:09:43.286848Z".to_string(),
                distribution_info: Some(DistributionInfo {
                    datetime: "2023-02-10T00:00:00Z".to_string(),
                    origin: None,
                }),
                created_datetime: "2024-04-17T05:09:42.777742Z".to_string(),
                last_written_datetime: "2024-04-17T05:09:43.284776Z".to_string(),
                is_folder: false,
                child_item_count: 0,
            },
        )]);

        let result = FilesystemSynchronizer::extract_paths_and_hashes(&input_json);
        assert_eq!(result, expected_output);
    }

    #[tokio::test]
    async fn test_extract_paths_and_hashes_with_null_distribution_info() {
        let input_json = json!({
            "child_folders": [
                {
                    "child_folders": [],
                    "child_items": [
                        {
                            "created_datetime": "2024-04-17T05:04:10.468603Z",
                            "distribution_info": {
                                "datetime": null,
                                "origin": null
                            },
                            "last_read_datetime": "2024-04-17T05:04:11.027076Z",
                            "last_written_datetime": "2024-04-17T05:04:11.024986Z",
                            "merkle_hash": "0c6adf07af938bf2dee144a4682234e4843f72f4249c9a1b6758bc9c8a999cbc",
                            "name": "why_82_of_mexico_is_empty",
                            "path": "/youtube/RealLifeLore/2023-02-18/why_82_of_mexico_is_empty",
                            "source_file_map_last_saved_datetime": "2024-04-17T05:04:11.027076Z",
                        }
                    ],
                    "created_datetime": "2024-04-17T05:03:23.035381Z",
                    "last_modified_datetime": "2024-04-17T05:04:11.027076Z",
                    "last_read_datetime": "2024-04-17T05:05:05.232397Z",
                    "last_written_datetime": "2024-04-17T05:04:11.027076Z",
                    "merkle_hash": "5517f968e0219dac509b73349e32f90a2fdc79ee467564341acaee493855336d",
                    "name": "2023-02-18",
                    "path": "/youtube/RealLifeLore/2023-02-18"
                }
            ],
            "child_items": [],
            "created_datetime": "2024-04-17T05:03:12.686786Z",
            "last_modified_datetime": "2024-04-17T05:03:23.035379Z",
            "last_read_datetime": "2024-04-17T05:05:06.560480Z",
            "last_written_datetime": "2024-04-17T05:04:11.045692Z",
            "merkle_hash": "7460feedd47d610c9ccb2955713eafb8c837916378d3e4bc6df7d4d5cb13f177",
            "name": "RealLifeLore",
            "path": "/youtube/RealLifeLore"
        });

        let expected_output = HashMap::from([
            (
                "/youtube/RealLifeLore/2023-02-18/why_82_of_mexico_is_empty".to_string(),
                FileInfo {
                    merkle_hash: "0c6adf07af938bf2dee144a4682234e4843f72f4249c9a1b6758bc9c8a999cbc".to_string(),
                    name: "why_82_of_mexico_is_empty".to_string(),
                    source_file_map_last_saved_datetime: "2024-04-17T05:04:11.027076Z".to_string(),
                    distribution_info: None,
                    created_datetime: "2024-04-17T05:04:10.468603Z".to_string(),
                    last_written_datetime: "2024-04-17T05:04:11.024986Z".to_string(),
                    is_folder: false,
                    child_item_count: 0,
                },
            ),
            (
                "/youtube/RealLifeLore/2023-02-18".to_string(),
                FileInfo {
                    merkle_hash: "5517f968e0219dac509b73349e32f90a2fdc79ee467564341acaee493855336d".to_string(),
                    name: "2023-02-18".to_string(),
                    source_file_map_last_saved_datetime: "".to_string(), // Assuming no specific datetime is provided
                    distribution_info: None,
                    created_datetime: "2024-04-17T05:03:23.035381Z".to_string(),
                    last_written_datetime: "2024-04-17T05:04:11.027076Z".to_string(),
                    is_folder: true,
                    child_item_count: 1,
                },
            ),
            (
                "/youtube/RealLifeLore".to_string(),
                FileInfo {
                    merkle_hash: "7460feedd47d610c9ccb2955713eafb8c837916378d3e4bc6df7d4d5cb13f177".to_string(),
                    name: "RealLifeLore".to_string(),
                    source_file_map_last_saved_datetime: "".to_string(), // Assuming no specific datetime is provided
                    distribution_info: None,
                    created_datetime: "2024-04-17T05:03:12.686786Z".to_string(),
                    last_written_datetime: "2024-04-17T05:04:11.045692Z".to_string(),
                    is_folder: true,
                    child_item_count: 1,
                },
            ),
        ]);

        let result = FilesystemSynchronizer::extract_paths_and_hashes(&input_json);
        assert_eq!(result, expected_output);
    }
}
