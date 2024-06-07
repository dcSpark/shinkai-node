use serde::{Deserialize, Serialize};
use shinkai_vector_resources::{
    resource_errors::VRError,
    vector_resource::{VRKai, VRPack},
};

#[derive(Deserialize, Serialize)]
pub struct VRPackContentItem {
    pub content: VRKai,
    pub path: String,
}

pub type VRPackContent = Vec<VRPackContentItem>;

pub trait ConvertFromVRPack {
    type Error;

    fn convert_from(vrpack: VRPack) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

impl ConvertFromVRPack for VRPackContent {
    type Error = VRError;

    fn convert_from(vrpack: VRPack) -> Result<Self, Self::Error> {
        let unpacked_vrkais = vrpack.unpack_all_vrkais()?;

        let content = unpacked_vrkais
            .into_iter()
            .map(|(vrkai, vrpath)| VRPackContentItem {
                content: vrkai,
                path: vrpath.format_to_string(),
            })
            .collect::<VRPackContent>();

        Ok(content)
    }
}
