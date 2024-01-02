use super::{Node, VectorResource};
use crate::resource_errors::VRError;

/// Trait extension which specific Vector Resource types implement that have a guaranteed internal ordering
/// of their nodes, such as DocumentVectorResources. This trait extension enables new
/// capabilities to be implemented, such as append/pop node interfaces, proximity searches, and more.
pub trait OrderedVectorResource: VectorResource {
    /// Id of the last node held internally
    fn last_node_id(&self) -> String;
    /// Id to be used when pushing a new node
    fn new_push_node_id(&self) -> String;
    /// Attempts to fetch a node (using the provided id) and proximity_window before/after, at root depth
    fn get_node_and_proximity(&self, id: String, proximity_window: u64) -> Result<Vec<Node>, VRError>;
}
