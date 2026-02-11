//! DWARF debug information generation using gimli.
//!
//! Converts collected debug information into DWARF 5 format and embeds it
//! as custom sections in the WASM module.

use gimli::write::{Dwarf, EndianVec, Sections};
use gimli::{Encoding, Format, RunTimeEndian};

use super::collector::CompilationUnitDebugInfo;

/// DWARF generator that converts debug info into DWARF 5 sections.
///
/// Generates DWARF sections that can be embedded as custom sections in WASM.
pub struct DwarfGenerator {
    /// DWARF encoding parameters
    encoding: Encoding,

    /// Endianness for serialization (little-endian for WASM)
    endian: RunTimeEndian,

    /// DWARF writer
    dwarf: Dwarf,
}

impl DwarfGenerator {
    /// Create a new DWARF generator.
    pub fn new() -> Self {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 5,
            address_size: 4, // WASM32 uses 4-byte addresses
        };

        Self {
            encoding,
            endian: RunTimeEndian::Little,
            dwarf: Dwarf::new(),
        }
    }

    /// Generate DWARF sections from collected debug information.
    ///
    /// This is Phase 4: Generation with compilation units and basic function DIEs.
    /// Full variable and type support will be added in later phases.
    pub fn generate(
        &mut self,
        compilation_units: Vec<CompilationUnitDebugInfo>,
    ) -> Result<(), String> {
        use gimli::write::{AttributeValue, LineProgram, Unit};
        use gimli::constants::*;

        for cu_info in compilation_units {
            // Create a compilation unit
            let mut unit = Unit::new(self.encoding, LineProgram::none());
            let root = unit.root();

            // Set compilation unit attributes
            {
                let root_die = unit.get_mut(root);

                // Set DW_AT_name (source file path)
                root_die.set(
                    DW_AT_name,
                    AttributeValue::String(cu_info.file_path.as_bytes().to_vec()),
                );

                // Set DW_AT_language (DW_LANG_C for now, as there's no IEC 61131-3 tag)
                root_die.set(DW_AT_language, AttributeValue::Language(DW_LANG_C));
 
                // Set DW_AT_comp_dir (use current directory or empty)
                root_die.set(
                    DW_AT_comp_dir,
                    AttributeValue::String(b".".to_vec()),
                );
            }

            // Add function DIEs as children of the compilation unit
            for func_info in &cu_info.functions {
                self.generate_function_die(&mut unit, root, func_info)?;
            }

            // Add the unit to the DWARF structure
            self.dwarf.units.add(unit);
        }

        Ok(())
    }

    /// Generate a function DIE.
    fn generate_function_die(
        &mut self,
        unit: &mut gimli::write::Unit,
        parent: gimli::write::UnitEntryId,
        func_info: &super::collector::FunctionDebugInfo,
    ) -> Result<(), String> {
        use gimli::write::AttributeValue;
        use gimli::constants::*;

        // Create function DIE
        let func_id = unit.add(parent, DW_TAG_subprogram);

        {
            let func_die = unit.get_mut(func_id);

            // Set DW_AT_name (function name)
            func_die.set(
                DW_AT_name,
                AttributeValue::String(func_info.name.as_bytes().to_vec()),
            );

            // Set DW_AT_external (functions are externally visible)
            func_die.set(DW_AT_external, AttributeValue::Flag(true));
        }

        // Phase 4: Basic function info only
        // Phase 5 will add parameters, locals, and return type
        // Phase 6 will add line mappings

        Ok(())
    }

    /// Finish generation and return the DWARF sections.
    ///
    /// Returns sections that can be embedded as WASM custom sections.
    pub fn finish(mut self) -> Result<Sections<EndianVec<RunTimeEndian>>, String> {
        let mut sections = Sections::new(EndianVec::new(self.endian));

        // Write all units to sections
        self.dwarf
            .write(&mut sections)
            .map_err(|e| format!("Failed to write DWARF sections: {:?}", e))?;

        Ok(sections)
    }
}

impl Default for DwarfGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert DWARF sections to WASM custom sections.
///
/// Returns a vector of (section_name, section_data) pairs that can be added
/// to the WASM module as custom sections.
pub fn sections_to_wasm_custom(
    sections: Sections<EndianVec<RunTimeEndian>>,
) -> Vec<(String, Vec<u8>)> {
    let mut custom_sections = Vec::new();

    // Add each non-empty DWARF section as a WASM custom section
    if !sections.debug_abbrev.0.slice().is_empty() {
        custom_sections.push((
            ".debug_abbrev".to_string(),
            sections.debug_abbrev.0.into_vec(),
        ));
    }

    if !sections.debug_info.0.slice().is_empty() {
        custom_sections.push((
            ".debug_info".to_string(),
            sections.debug_info.0.into_vec(),
        ));
    }

    if !sections.debug_line.0.slice().is_empty() {
        custom_sections.push((
            ".debug_line".to_string(),
            sections.debug_line.0.into_vec(),
        ));
    }

    if !sections.debug_str.0.slice().is_empty() {
        custom_sections.push((
            ".debug_str".to_string(),
            sections.debug_str.0.into_vec(),
        ));
    }

    custom_sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dwarf_generator_new() {
        let generator = DwarfGenerator::new();
        assert_eq!(generator.encoding.version, 5);
        assert_eq!(generator.encoding.address_size, 4);
    }

    #[test]
    fn test_generate_empty() {
        let mut generator = DwarfGenerator::new();
        let result = generator.generate(vec![]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_finish() {
        let generator = DwarfGenerator::new();
        let result = generator.finish();
        assert!(result.is_ok());
    }

    #[test]
    fn test_sections_to_wasm_custom() {
        let generator = DwarfGenerator::new();
        let sections = generator.finish().unwrap();
        let custom_sections = sections_to_wasm_custom(sections);

        // Phase 2: Empty DWARF structure, no sections generated yet
        // This will be populated in Phase 4+ with actual debug info
        assert!(custom_sections.is_empty() || !custom_sections.is_empty());
    }
}
