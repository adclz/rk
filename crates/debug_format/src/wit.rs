//! The WIT packages a component built by rk refers to, as text: rk's own
//! control interface, which every component exports, and the WASI packages
//! the standard library imports from, with those they name, as WASI's own
//! repository writes them (the git submodule at `wit/wasi`). The compiler writes the ones a component
//! names beside it, under `wit/deps/`, where a host's bindings generator
//! reads them; a host may also take them from here.

/// A WIT package and the files it is written in.
pub struct Package {
    /// The name and version its files declare: `wasi:clocks@0.3.0`.
    pub id: &'static str,
    /// The folder it is written in under `wit/deps/`.
    pub dir: &'static str,
    /// Each file, by name, with its text.
    pub files: &'static [(&'static str, &'static str)],
    /// The terms the package is published under, when they are not rk's.
    pub license: Option<&'static str>,
}

/// The control interface, with its version, as a component exports it.
pub const CONTROL_INTERFACE: &str = "rk:plc/control@0.1.0";

/// rk's package: the control interface.
pub static RK_PLC: Package = Package {
    id: "rk:plc@0.1.0",
    dir: "rk-plc",
    files: &[("control.wit", include_str!("../wit/rk-plc/control.wit"))],
    license: None,
};

/// The WASI packages rk carries, at the release the standard library
/// imports, as `build.rs` finds them in WASI's repository.
pub static WASI: &[Package] = include!(concat!(env!("OUT_DIR"), "/wasi.rs"));

/// Every package rk carries: its own, then WASI's.
pub fn packages() -> impl Iterator<Item = &'static Package> {
    std::iter::once(&RK_PLC).chain(WASI)
}

/// The package an interface belongs to, by the interface's name and
/// version: `wasi:clocks/monotonic-clock@0.3.0` is in `wasi:clocks@0.3.0`.
/// `None` when rk carries no such package at that version.
pub fn package_of(interface: &str) -> Option<&'static Package> {
    let (package, rest) = interface.split_once('/')?;
    let (_, version) = rest.split_once('@')?;
    let id = format!("{package}@{version}");
    packages().find(|candidate| candidate.id == id)
}

/// An interface of a WASI package rk carries, with the version rk carries
/// it at: `interface_of("wasi:cli", "stdout")` is `wasi:cli/stdout@0.3.0`.
/// What the compiler imports of its own accord is named this way, so that
/// it follows the release WASI's repository is checked out at.
pub fn interface_of(package: &str, interface: &str) -> Option<String> {
    let carried = WASI
        .iter()
        .find(|carried| carried.id.split('@').next() == Some(package))?;
    let (_, version) = carried.id.split_once('@')?;
    Some(format!("{package}/{interface}@{version}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each package's files declare its name and version, the ones that
    /// declare any, and the control interface's name is the one its file
    /// declares.
    #[test]
    fn each_package_declares_its_name() {
        for package in packages() {
            let declared: Vec<&str> = package
                .files
                .iter()
                .flat_map(|(_, text)| text.lines())
                .filter_map(|line| line.trim().strip_prefix("package ")?.strip_suffix(';'))
                .collect();
            assert!(!declared.is_empty(), "{} declares no package", package.dir);
            assert!(
                declared.iter().all(|declared| *declared == package.id),
                "{} declares {declared:?}, not {}",
                package.dir,
                package.id
            );
        }
        let (package, rest) = CONTROL_INTERFACE.split_once('/').unwrap();
        let (interface, version) = rest.split_once('@').unwrap();
        let text = RK_PLC.files[0].1;
        assert!(text.contains(&format!("package {package}@{version};")));
        assert!(text.contains(&format!("interface {interface} {{")));
    }

    /// An interface is found in its package at its own version only: the
    /// clocks rk carries are WASI 0.3.0's, with WASI's license.
    #[test]
    fn an_interface_is_found_at_its_version() {
        let clocks = package_of("wasi:clocks/monotonic-clock@0.3.0").expect("the clocks");
        assert_eq!(clocks.id, "wasi:clocks@0.3.0");
        assert_eq!(clocks.dir, "wasi-clocks");
        assert!(clocks.license.is_some_and(|text| text.contains("WASI")));
        assert_eq!(package_of(CONTROL_INTERFACE).map(|p| p.id), Some(RK_PLC.id));
        // The command line's interfaces, and the packages its files name.
        let carried: Vec<&str> = WASI.iter().map(|package| package.id).collect();
        assert_eq!(
            carried,
            [
                "wasi:cli@0.3.0",
                "wasi:clocks@0.3.0",
                "wasi:filesystem@0.3.0",
                "wasi:random@0.3.0",
                "wasi:sockets@0.3.0"
            ]
        );
        assert_eq!(
            interface_of("wasi:cli", "stdout").as_deref(),
            Some("wasi:cli/stdout@0.3.0")
        );
        assert_eq!(interface_of("wasi:http", "types"), None);
        for unknown in [
            "wasi:clocks/monotonic-clock@0.2.6",
            "wasi:clocks/monotonic-clock",
            "wasi:io/poll@0.3.0",
            "host",
        ] {
            assert!(package_of(unknown).is_none(), "{unknown}");
        }
    }
}
