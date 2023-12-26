use super::permissions::PermissionsIndex;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_search_traversal::VRPath;

pub struct VFSWriter {
    requester_name: ShinkaiName,
    path: VRPath,
}
