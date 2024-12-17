use async_trait::async_trait;
use shinkai_message_primitives::schemas::identity::Identity;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

#[async_trait]
pub trait IdentityManagerTrait: Send + Sync {
    fn find_by_identity_name(&self, full_profile_name: ShinkaiName) -> Option<&Identity>;
    async fn search_identity(&self, full_identity_name: &str) -> Option<Identity>;
}
