use super::{Node, VectorResource};
use crate::{embeddings::Embedding, resource_errors::VRError};

/// Trait extension which specific Vector Resource types implement that have a guaranteed internal ordering
/// of their nodes, such as DocumentVectorResources. This trait extension enables new
/// capabilities to be implemented, such as append/pop node interfaces, proximity searches, and more.
pub trait OrderedVectorResource: VectorResource {
    /// Id of the first node held internally
    fn first_node_id(&self) -> Option<String>;
    /// Id of the last node held internally
    fn last_node_id(&self) -> Option<String>;
    /// Retrieve the first node held internally
    fn get_first_node(&self) -> Option<Node>;
    /// Retrieve the second node held internally
    fn get_second_node(&self) -> Option<Node>;
    /// Retrieve the third node held internally
    fn get_third_node(&self) -> Option<Node>;
    /// Retrieve the last node held internally
    fn get_last_node(&self) -> Option<Node>;
    /// Id to be used when pushing a new node
    fn new_push_node_id(&self) -> String;
    /// Takes the first N nodes held internally and returns them as references
    fn take(&self, n: usize) -> Vec<&Node>;
    /// Takes the first N nodes held internally and returns cloned copies of them
    fn take_cloned(&self, n: usize) -> Vec<Node>;
    /// Attempts to fetch a node (using the provided id) and proximity_window before/after, at root depth
    fn get_node_and_embedding_proximity(
        &self,
        id: String,
        proximity_window: u64,
    ) -> Result<Vec<(Node, Embedding)>, VRError>;
}
