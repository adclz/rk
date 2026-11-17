//! The WIT packages rk declares itself, as text: the control interface
//! every component exports, and the interface only the module `rk test`
//! builds imports. Nothing of WASI's is here: the compiler carries WASI's
//! packages, and a library ships its own in its `wit/` folder. The
//! compiler writes the packages a component names beside it, under
//! `wit/deps/`, where a host's bindings generator reads them; a host may
//! also take rk's own from here.

/// A WIT package and the files it is written in.
pub struct Package {
    /// The name and version its files declare: `rk:plc@0.1.0`.
    pub id: &'static str,
    /// The folder it is written in under `wit/deps/`.
    pub dir: &'static str,
    /// Each file, by name, with its text.
    pub files: &'static [(&'static str, &'static str)],
}

/// The control interface, with its version, as a component exports it.
pub const CONTROL_INTERFACE: &str = "rk:plc/control@0.1.0";

/// The interface only the module `rk test` builds imports, with its
/// version: the host is told where each raise is.
pub const TEST_INTERFACE: &str = "rk:test/raise@0.1.0";

/// rk's package for a host: the control interface.
pub static RK_PLC: Package = Package {
    id: "rk:plc@0.1.0",
    dir: "rk-plc",
    files: &[("control.wit", include_str!("../wit/rk-plc/control.wit"))],
};

/// rk's package for the host that runs the tests: the test interface.
pub static RK_TEST: Package = Package {
    id: "rk:test@0.1.0",
    dir: "rk-test",
    files: &[("raise.wit", include_str!("../wit/rk-test/raise.wit"))],
};

/// Every package rk declares itself.
pub fn packages() -> impl Iterator<Item = &'static Package> {
    [&RK_PLC, &RK_TEST].into_iter()
}

/// The package of rk's an interface belongs to, by the interface's name and
/// version: `rk:plc/control@0.1.0` is in `rk:plc@0.1.0`. `None` when rk
/// declares no such package at that version.
pub fn package_of(interface: &str) -> Option<&'static Package> {
    let (package, rest) = interface.split_once('/')?;
    let (_, version) = rest.split_once('@')?;
    let id = format!("{package}@{version}");
    packages().find(|candidate| candidate.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each package's files declare its name and version, the ones that
    /// declare any, and rk's two interfaces are named as their files declare
    /// them.
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
        for (name, carried) in [(CONTROL_INTERFACE, &RK_PLC), (TEST_INTERFACE, &RK_TEST)] {
            let (package, rest) = name.split_once('/').unwrap();
            let (interface, version) = rest.split_once('@').unwrap();
            let text = carried.files[0].1;
            assert!(text.contains(&format!("package {package}@{version};")));
            assert!(text.contains(&format!("interface {interface} {{")));
        }
    }

    /// An interface is found in its package at its own version only, and
    /// WASI's are no packages of rk's.
    #[test]
    fn an_interface_is_found_at_its_version() {
        assert_eq!(package_of(CONTROL_INTERFACE).map(|p| p.id), Some(RK_PLC.id));
        assert_eq!(package_of(TEST_INTERFACE).map(|p| p.id), Some(RK_TEST.id));
        for unknown in [
            "rk:plc/control@0.2.0",
            "rk:plc/control",
            "wasi:clocks/monotonic-clock@0.3.0",
            "host",
        ] {
            assert!(package_of(unknown).is_none(), "{unknown}");
        }
    }
}
