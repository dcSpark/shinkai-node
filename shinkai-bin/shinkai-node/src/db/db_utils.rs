


use super::{db_errors::ShinkaiDBError, ShinkaiDB};

impl ShinkaiDB {
    #[cfg(debug_assertions)]
    pub fn print_all_from_cf(&self, _inbox: &str) -> Result<(), ShinkaiDBError> {
        // Not used anymore bc not applicable with the new structure
        // TODO: update to something different
        Ok(())
    }
}
