#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Edition {
    Community,
    Pro,
    Team,
    Enterprise,
}

impl Edition {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Community  => "Community",
            Self::Pro        => "Pro",
            Self::Team       => "Team",
            Self::Enterprise => "Enterprise",
        }
    }

    /// Returns true if this edition has access to the requested edition's features.
    pub fn can(&self, required: Edition) -> bool {
        *self >= required
    }
}