use super::internals::VectorFSInternals;

pub struct VectorFS {
    internals: VectorFSInternals,
    // db: Arc<Mutex<VectorFSDB>>,
}
