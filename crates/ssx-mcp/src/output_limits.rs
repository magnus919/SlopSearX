//! Trusted composition-only response budgets. Input limits are independent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputLimits {
    bytes: usize,
}
impl OutputLimits {
    pub const HTTP: Self = Self {
        bytes: 4 * 1024 * 1024,
    };
    pub const LEGACY: Self = Self {
        bytes: 16 * 1024 * 1024,
    };
    /// The finite ceiling cannot exceed legacy's existing aggregate output
    /// ownership budget. No request field, descriptor or caller metadata selects it.
    pub fn new(bytes: usize) -> Result<Self, &'static str> {
        if bytes == 0 || bytes > 64 * 1024 * 1024 {
            return Err("invalid native response byte budget");
        }
        Ok(Self { bytes })
    }
    pub fn bytes(self) -> usize {
        self.bytes
    }
}
