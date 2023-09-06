/// A struct that represents the engine id of a node.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(u32);

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // write!(f, "{:#010x}", self.0)
        write!(f, "{}", self.0)
    }
}

impl PartialEq<u32> for Id {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<u32> for Id {
    fn partial_cmp(&self, other: &u32) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Id {
    /// Creates a new Id from a u32.
    pub fn new(id: u32) -> Self {
        Id(id)
    }

    /// Returns the u32 data of the Id.
    pub fn data(&self) -> u32 {
        self.0
    }
}

impl From<u32> for Id {
    fn from(value: u32) -> Self {
        Id(value)
    }
}

/// A struct that represents a TCP port number.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TcpPort(u32);

impl std::fmt::Display for TcpPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialEq<u32> for TcpPort {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<u32> for TcpPort {
    fn partial_cmp(&self, other: &u32) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl TcpPort {
    /// Creates a new TcpPort from a u32.
    pub fn new(port: u32) -> Self {
        TcpPort(port)
    }

    /// Returns the u32 data of the TcpPort.
    pub fn data(&self) -> u32 {
        self.0
    }
}

impl From<u32> for TcpPort {
    fn from(value: u32) -> Self {
        TcpPort(value)
    }
}

/// A struct that represents the hop limit of a node.
/// This value must be in the range [0,7].
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HopLimit(u8);

impl std::fmt::Display for HopLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for HopLimit {
    fn default() -> Self {
        HopLimit(3)
    }
}

impl PartialEq<u8> for HopLimit {
    fn eq(&self, other: &u8) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<u8> for HopLimit {
    fn partial_cmp(&self, other: &u8) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl HopLimit {
    /// Creates a new HopLimit from a u8.
    /// Returns an error if the value is not in the range [0,7].
    pub fn new(hop_limit: u8) -> Result<Self, String> {
        if hop_limit > 7 {
            return Err("Hop limit must be in range [0,7]".to_string());
        }

        Ok(HopLimit(hop_limit))
    }

    /// Returns the u8 data of the HopLimit.
    pub fn data(&self) -> u8 {
        self.0
    }
}

impl From<u8> for HopLimit {
    fn from(value: u8) -> Self {
        HopLimit(value)
    }
}
