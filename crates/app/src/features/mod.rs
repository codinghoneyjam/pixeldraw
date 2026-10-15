//! Feature facades (discoverability only, phase 1).

pub mod arc {
    pub use crate::tools::arc;
    pub use crate::tools::arc::*;
}

pub mod gradient {
    pub use crate::tools::gradient;
    pub use crate::tools::gradient::*;
}

pub mod pen {
    pub use crate::tools::pen;
    pub use crate::tools::pen::*;
}

pub mod shape {
    pub use crate::tools::shape;
    pub use crate::tools::shape::*;
}

pub mod eyedropper {
    pub use crate::tools::eyedropper;
    pub use crate::tools::eyedropper::*;
}

pub mod fill {
    pub use crate::tools::fill;
    pub use crate::tools::fill::*;
}

pub mod hand {
    pub use crate::tools::hand;
    pub use crate::tools::hand::*;
}

pub mod text {
    pub use crate::tools::text;
    pub use crate::tools::text::*;
}