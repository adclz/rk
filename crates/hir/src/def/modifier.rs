use bitflags::bitflags;

bitflags! {
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifier: u16 {
        const ABSTRACT = 1 << 0;
        const FINAL = 1 << 1;
        const OVERRIDE = 1 << 2;
    }
}

impl Modifier {
    pub const EMPTY: Modifier = Modifier::empty();
}
