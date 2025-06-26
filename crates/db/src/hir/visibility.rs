use bitflags::bitflags;

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u16 {
        const PUBLIC = 1 << 0;
        const PROTECTED = 1 << 1;
        const INTERNAL = 1 << 2;
        const PRIVATE = 1 << 3;

        const IMPLEMENTS = 1 << 4;
        const EXTENDS = 1 << 5;

        const ABSTRACT = 1 << 6;
        const FINAL = 1 << 7;
        const OVERRIDE = 1 << 8;
    }
}