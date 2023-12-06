use crate::base_vector_resources::BaseVectorResource;
use crate::base_vector_resources::VRBaseType;
use crate::data_tags::DataTag;
use crate::data_tags::DataTagIndex;
use crate::embedding_generator::EmbeddingGenerator;
#[cfg(feature = "native-http")]
use crate::embedding_generator::RemoteEmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::embeddings::MAX_EMBEDDING_STRING_SIZE;
use crate::model_type::EmbeddingModelType;
use crate::resource_errors::VRError;
use crate::shinkai_time::ShinkaiTime;
use crate::source::VRLocation;
use crate::source::VRSource;
pub use crate::vector_resource_types::*;
use async_trait::async_trait;
use env_logger::filter::Filter;

/// An enum that represents the different traversal approaches
/// supported by Vector Searching. In other words these allow the developer to
/// choose how the searching algorithm
#[derive(Debug, Clone, PartialEq)]
pub enum TraversalMethod {
    /// Efficiently only goes deeper into Vector Resources if they are the highest scored Nodes at their level.
    /// Will go infinitely deep until hitting a level where no BaseVectorResources are part of the highest scored.
    Efficient,
    /// Traverses through all levels of depth and scores all content holding nodes.
    Exhaustive,
    /// Iterates exhaustively going through all levels while doing absolutely no scoring/similarity checking,
    /// returning every single Node at any level. Also returns the Vector Resources in addition to their
    /// Nodes they hold inside, thus providing all nodes that exist within the root Vector Resource.
    /// Note: This is not for vector searching, but for retrieving all possible Nodes.
    UnscoredAllNodes,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TraversalOption {
    /// Limits traversal into deeper Vector Resources only if they match the provided VRBaseType
    LimitTraversalToType(VRBaseType),
    /// Limits returned result to be within a percentage range (0.0 - 1.0) of the highest scored result.
    /// For example, you can set a tolerance range of 0.1 which means only nodes with a similarity score
    /// within 10% of the top result will be returned.
    ToleranceRangeResults(f32),
    /// Limits returned results to be greater than a specific score (0.0 - 1.0)
    MinimumScore(f32),
    /// Efficiently traverses until (and including) the specified depth is hit (or until there are no more levels to go).
    /// Will return BaseVectorResource Nodes if they are the highest scored at the specified depth.
    /// Top/root level starts at 0, and so first level of depth into internal BaseVectorResources is thus 1.
    UntilDepth(u64),
    /// By default Vector Search scoring only weighs a node based on it's single embedding alone.
    /// Alternate scoring modes are available which allow weighing a node base on relative scores
    /// above/below/beside, or otherwise to get potentially higher quality results.
    SetScoringMode(ScoringMode),
    /// Set a prefilter mode while performing a vector search. These modes use pre-processed indices in the Vector Resource
    /// to efficiently filter out all unrelated nodes before performing any semantic search logic.
    SetPrefilterMode(PrefilterMode),
    /// Set a filter mode while performing a vector search. These modes allow filtering elements during a Vector Search
    /// dynamically based on data within each found node. They do not use an indices, so are slower than prefiler modes.
    SetFilterMode(FilterMode),
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ScoringMode {
    /// While traversing, averages out the score all the way to each final node. In other words, the final score
    /// of each node weighs-in the scores of the Vector Resources that it was inside all the way up to the root.
    HierarchicalAverageScoring,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrefilterMode {
    /// Perform a Syntactic Vector Search.
    /// A syntactic vector search efficiently pre-filters all Nodes held internally to a subset that
    /// matches the provided list of data tag names (Strings).
    SyntacticVectorSearch(Vec<String>),
}

pub type Key = String;
pub type Value = String;
#[derive(Debug, Clone, PartialEq)]
pub enum FilterMode {
    /// Filters out Nodes which do not match at least one of the (Key, Option<Value>) pairs in the list.
    /// Note, if Value is `None`, then we only check that the Node has a matching key, with the value being ignored.
    ContainsAnyMetadataKeyValues(Vec<(Key, Option<Value>)>),
    /// Filters out Nodes which do not match all of the (Key, Option<Value>) pairs in the list.
    /// Note, if Value is `None`, then we only check that the Node has a matching key, with the value being ignored.
    ContainsAllMetadataKeyValues(Vec<(Key, Option<Value>)>),
}

impl FilterMode {
    /// Helper function to check if a node contains any matching key values
    pub fn node_metadata_any_check(node: &Node, kv_pairs: &Vec<(Key, Option<Value>)>) -> bool {
        if let Some(metadata) = &node.metadata {
            for (key, value_option) in kv_pairs {
                if let Some(value) = metadata.get(key) {
                    if value_option.is_none() {
                        return true;
                    } else if let Some(expected_value) = value_option {
                        if value == expected_value {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Helper function to check if a node contains all matching key values
    pub fn node_metadata_all_check(node: &Node, kv_pairs: &Vec<(Key, Option<Value>)>) -> bool {
        if let Some(metadata) = &node.metadata {
            for (key, value_option) in kv_pairs {
                if let Some(value) = metadata.get(key) {
                    if let Some(expected_value) = value_option {
                        if value != expected_value {
                            return false;
                        }
                    }
                } else {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }
}

pub trait TraversalOptionVecExt {
    fn get_limit_traversal_to_type_option(&self) -> Option<&VRBaseType>;
    fn get_tolerance_range_results_option(&self) -> Option<f32>;
    fn get_minimum_score_option(&self) -> Option<f32>;
    fn get_until_depth_option(&self) -> Option<u64>;
    fn get_set_scoring_mode_option(&self) -> Option<ScoringMode>;
    fn get_set_prefilter_mode_option(&self) -> Option<PrefilterMode>;
    fn get_set_filter_mode_option(&self) -> Option<FilterMode>;
}

impl TraversalOptionVecExt for Vec<TraversalOption> {
    fn get_limit_traversal_to_type_option(&self) -> Option<&VRBaseType> {
        self.iter().find_map(|option| {
            if let TraversalOption::LimitTraversalToType(value) = option {
                Some(value)
            } else {
                None
            }
        })
    }

    fn get_tolerance_range_results_option(&self) -> Option<f32> {
        self.iter().find_map(|option| {
            if let TraversalOption::ToleranceRangeResults(value) = option {
                Some(*value)
            } else {
                None
            }
        })
    }

    fn get_minimum_score_option(&self) -> Option<f32> {
        self.iter().find_map(|option| {
            if let TraversalOption::MinimumScore(value) = option {
                Some(*value)
            } else {
                None
            }
        })
    }

    fn get_until_depth_option(&self) -> Option<u64> {
        self.iter().find_map(|option| {
            if let TraversalOption::UntilDepth(value) = option {
                Some(*value)
            } else {
                None
            }
        })
    }

    fn get_set_scoring_mode_option(&self) -> Option<ScoringMode> {
        self.iter().find_map(|option| {
            if let TraversalOption::SetScoringMode(value) = option {
                Some(*value)
            } else {
                None
            }
        })
    }

    fn get_set_prefilter_mode_option(&self) -> Option<PrefilterMode> {
        self.iter().find_map(|option| {
            if let TraversalOption::SetPrefilterMode(value) = option {
                Some(value.clone())
            } else {
                None
            }
        })
    }

    fn get_set_filter_mode_option(&self) -> Option<FilterMode> {
        self.iter().find_map(|option| {
            if let TraversalOption::SetFilterMode(value) = option {
                Some(value.clone())
            } else {
                None
            }
        })
    }
}
