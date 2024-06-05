//! This file contains the enum `Error` for all the errors returned by the
//! module `histogram`.

use plotters::drawing::DrawingAreaErrorKind;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// `Drawing` variant is for errors reported by the crate `plotters`.
    #[error(transparent)]
    Drawing(#[from] DrawingAreaErrorKind<std::io::Error>),

    #[error("The file {0} exists already. To ignore it, pass the flag --overwrite.")]
    FileExists(String),

    /// `Save` variant is for errors reported when saving the SVG image to file.
    #[error(transparent)]
    Save(#[from] std::io::Error),

    #[error("The list of `libfuncs` called is empty. Can't create histogram.")]
    Empty,

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error generating libfunc histogram: {0:?}")]
    Unknown(String),
}
