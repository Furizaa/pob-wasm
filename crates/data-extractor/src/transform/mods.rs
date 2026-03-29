use crate::{error::ExtractError, ggpk_reader::GgpkReader};
use std::path::Path;

pub fn extract(_reader: &GgpkReader, _output: &Path) -> Result<(), ExtractError> {
    Ok(())
}
