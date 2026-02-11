//! Type conversion from HIR types to DWARF debug types.

use hir::hir_def::expressions::spec::ElementarySpec;

use super::collector::DwarfEncoding;

/// Convert an IEC 61131-3 elementary type to DWARF encoding and byte size.
///
/// Returns `(DwarfEncoding, byte_size)` for the given elementary type.
///
/// # Examples
///
/// ```rust,ignore
/// let (encoding, size) = elementary_to_dwarf(ElementarySpec::Int);
/// assert_eq!(encoding, DwarfEncoding::Signed);
/// assert_eq!(size, 2); // INT is 16 bits
/// ```
pub fn elementary_to_dwarf(spec: ElementarySpec) -> (DwarfEncoding, u8) {
    match spec {
        // Boolean types (1 byte)
        ElementarySpec::Bool => (DwarfEncoding::Boolean, 1),
        ElementarySpec::REDGEBool => (DwarfEncoding::Boolean, 1),
        ElementarySpec::FEDGEBool => (DwarfEncoding::Boolean, 1),

        // Bitstring types (unsigned)
        ElementarySpec::Byte => (DwarfEncoding::Unsigned, 1),
        ElementarySpec::Word => (DwarfEncoding::Unsigned, 2),
        ElementarySpec::DWord => (DwarfEncoding::Unsigned, 4),
        ElementarySpec::LWord => (DwarfEncoding::Unsigned, 8),

        // Signed integer types
        ElementarySpec::SInt => (DwarfEncoding::Signed, 1),   // SINT: 8-bit signed
        ElementarySpec::Int => (DwarfEncoding::Signed, 2),    // INT: 16-bit signed
        ElementarySpec::DInt => (DwarfEncoding::Signed, 4),   // DINT: 32-bit signed
        ElementarySpec::LInt => (DwarfEncoding::Signed, 8),   // LINT: 64-bit signed

        // Unsigned integer types
        ElementarySpec::USInt => (DwarfEncoding::Unsigned, 1), // USINT: 8-bit unsigned
        ElementarySpec::UInt => (DwarfEncoding::Unsigned, 2),  // UINT: 16-bit unsigned
        ElementarySpec::UDInt => (DwarfEncoding::Unsigned, 4), // UDINT: 32-bit unsigned
        ElementarySpec::ULInt => (DwarfEncoding::Unsigned, 8), // ULINT: 64-bit unsigned

        // Floating-point types
        ElementarySpec::Real => (DwarfEncoding::Float, 4),  // REAL: 32-bit float
        ElementarySpec::LReal => (DwarfEncoding::Float, 8), // LREAL: 64-bit float

        // String types (variable-length, default to 80 bytes)
        // In practice, these should be sized based on declarations like STRING[80]
        // Using 80 as a default since STRING[80] is a common declaration
        ElementarySpec::String => (DwarfEncoding::Unsigned, 80),
        ElementarySpec::WString => (DwarfEncoding::Unsigned, 160), // Wide string (2 bytes per char)

        // Character types
        ElementarySpec::Char => (DwarfEncoding::Unsigned, 1),
        ElementarySpec::WChar => (DwarfEncoding::Unsigned, 2),

        // Date and time types (typically 64-bit signed nanoseconds/milliseconds)
        ElementarySpec::Date => (DwarfEncoding::Signed, 8),
        ElementarySpec::LDate => (DwarfEncoding::Signed, 8),
        ElementarySpec::DateAndTime => (DwarfEncoding::Signed, 8),
        ElementarySpec::LDateTime => (DwarfEncoding::Signed, 8),

        // Duration types (signed 64-bit nanoseconds)
        ElementarySpec::Time => (DwarfEncoding::Signed, 8),
        ElementarySpec::LTime => (DwarfEncoding::Signed, 8),

        // Time of day types (signed 64-bit nanoseconds since midnight)
        ElementarySpec::Tod => (DwarfEncoding::Signed, 8),
        ElementarySpec::LTod => (DwarfEncoding::Signed, 8),
    }
}

/// Get the IEC 61131-3 type name for an elementary type.
///
/// Returns the canonical name as used in source code (e.g., "INT", "REAL", "BOOL").
pub fn elementary_type_name(spec: ElementarySpec) -> &'static str {
    match spec {
        ElementarySpec::Bool => "BOOL",
        ElementarySpec::REDGEBool => "R_EDGE",
        ElementarySpec::FEDGEBool => "F_EDGE",
        ElementarySpec::Byte => "BYTE",
        ElementarySpec::Word => "WORD",
        ElementarySpec::DWord => "DWORD",
        ElementarySpec::LWord => "LWORD",
        ElementarySpec::SInt => "SINT",
        ElementarySpec::USInt => "USINT",
        ElementarySpec::UInt => "UINT",
        ElementarySpec::Int => "INT",
        ElementarySpec::DInt => "DINT",
        ElementarySpec::UDInt => "UDINT",
        ElementarySpec::LInt => "LINT",
        ElementarySpec::ULInt => "ULINT",
        ElementarySpec::Real => "REAL",
        ElementarySpec::LReal => "LREAL",
        ElementarySpec::String => "STRING",
        ElementarySpec::WString => "WSTRING",
        ElementarySpec::Char => "CHAR",
        ElementarySpec::WChar => "WCHAR",
        ElementarySpec::Date => "DATE",
        ElementarySpec::LDate => "LDATE",
        ElementarySpec::DateAndTime => "DATE_AND_TIME",
        ElementarySpec::LDateTime => "LDATE_AND_TIME",
        ElementarySpec::Time => "TIME",
        ElementarySpec::LTime => "LTIME",
        ElementarySpec::Tod => "TIME_OF_DAY",
        ElementarySpec::LTod => "LTIME_OF_DAY",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boolean_types() {
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::Bool);
        assert_eq!(encoding, DwarfEncoding::Boolean);
        assert_eq!(size, 1);

        assert_eq!(elementary_type_name(ElementarySpec::Bool), "BOOL");
    }

    #[test]
    fn test_signed_integers() {
        // SINT: 8-bit
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::SInt);
        assert_eq!(encoding, DwarfEncoding::Signed);
        assert_eq!(size, 1);

        // INT: 16-bit
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::Int);
        assert_eq!(encoding, DwarfEncoding::Signed);
        assert_eq!(size, 2);

        // DINT: 32-bit
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::DInt);
        assert_eq!(encoding, DwarfEncoding::Signed);
        assert_eq!(size, 4);

        // LINT: 64-bit
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::LInt);
        assert_eq!(encoding, DwarfEncoding::Signed);
        assert_eq!(size, 8);
    }

    #[test]
    fn test_unsigned_integers() {
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::USInt);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 1);

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::UInt);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 2);

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::UDInt);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 4);

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::ULInt);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 8);
    }

    #[test]
    fn test_float_types() {
        // REAL: 32-bit float
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::Real);
        assert_eq!(encoding, DwarfEncoding::Float);
        assert_eq!(size, 4);

        // LREAL: 64-bit float
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::LReal);
        assert_eq!(encoding, DwarfEncoding::Float);
        assert_eq!(size, 8);

        assert_eq!(elementary_type_name(ElementarySpec::Real), "REAL");
        assert_eq!(elementary_type_name(ElementarySpec::LReal), "LREAL");
    }

    #[test]
    fn test_bitstring_types() {
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::Byte);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 1);

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::Word);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 2);

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::DWord);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 4);

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::LWord);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 8);
    }

    #[test]
    fn test_time_types() {
        // All time types are 64-bit signed
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::Time);
        assert_eq!(encoding, DwarfEncoding::Signed);
        assert_eq!(size, 8);

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::Date);
        assert_eq!(encoding, DwarfEncoding::Signed);
        assert_eq!(size, 8);

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::DateAndTime);
        assert_eq!(encoding, DwarfEncoding::Signed);
        assert_eq!(size, 8);

        assert_eq!(elementary_type_name(ElementarySpec::Time), "TIME");
        assert_eq!(elementary_type_name(ElementarySpec::Date), "DATE");
    }

    #[test]
    fn test_string_types() {
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::String);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 80); // Default size (STRING[80])

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::WString);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 160); // Wide string default (2 bytes per char)

        assert_eq!(elementary_type_name(ElementarySpec::String), "STRING");
        assert_eq!(elementary_type_name(ElementarySpec::WString), "WSTRING");
    }

    #[test]
    fn test_char_types() {
        let (encoding, size) = elementary_to_dwarf(ElementarySpec::Char);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 1);

        let (encoding, size) = elementary_to_dwarf(ElementarySpec::WChar);
        assert_eq!(encoding, DwarfEncoding::Unsigned);
        assert_eq!(size, 2);
    }
}
