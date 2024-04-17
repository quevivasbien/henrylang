#[derive(Copy, Clone)]
pub union Value {
    pub i: i64,
    pub f: f64,
    pub b: bool,
}

impl Value {
    pub fn from_i64(i: i64) -> Self {
        Self { i }
    }
    pub fn from_f64(f: f64) -> Self {
        Self { f }
    }
    pub fn from_bool(b: bool) -> Self {
        Self { b }
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let constant = unsafe {
            std::mem::transmute::<&Self, &u64>(self)
        };
        write!(f, "{:#x?}", constant)
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        unsafe {
            self.i == other.i
        }
    }
}