//! The WIT packages a component built by rk refers to, as text: rk's own
//! control interface, which every component exports, and the WASI packages
//! the standard library imports. The compiler writes them beside the
//! component, under `wit/deps/`, where a host's bindings generator reads
//! them; a host may also take them from here.

/// A WIT package and the files it is written in.
pub struct Package {
    /// The name its files declare, without the version: `rk:plc`.
    pub name: &'static str,
    /// The folder it is written in under `wit/deps/`.
    pub dir: &'static str,
    /// Each file, by name, with its text.
    pub files: &'static [(&'static str, &'static str)],
    /// The terms the package is published under, when they are not rk's.
    pub license: Option<&'static str>,
    /// The packages its files refer to.
    pub uses: &'static [&'static str],
}

/// The control interface, with its version, as a component exports it.
pub const CONTROL_INTERFACE: &str = "rk:plc/control@0.1.0";

/// rk's package: the control interface.
pub const RK_PLC: Package = Package {
    name: "rk:plc",
    dir: "rk-plc",
    files: &[("control.wit", include_str!("../wit/rk-plc/control.wit"))],
    license: None,
    uses: &[],
};

/// WASI's clocks, at the version the standard library imports.
pub const WASI_CLOCKS: Package = Package {
    name: "wasi:clocks",
    dir: "wasi-clocks",
    files: &[
        (
            "monotonic-clock.wit",
            include_str!("../wit/wasi-clocks/monotonic-clock.wit"),
        ),
        (
            "timezone.wit",
            include_str!("../wit/wasi-clocks/timezone.wit"),
        ),
        (
            "wall-clock.wit",
            include_str!("../wit/wasi-clocks/wall-clock.wit"),
        ),
        ("world.wit", include_str!("../wit/wasi-clocks/world.wit")),
    ],
    license: Some(include_str!("../wit/wasi-clocks/LICENSE.md")),
    uses: &["wasi:io"],
};

/// WASI's I/O, which the clocks refer to.
pub const WASI_IO: Package = Package {
    name: "wasi:io",
    dir: "wasi-io",
    files: &[
        ("error.wit", include_str!("../wit/wasi-io/error.wit")),
        ("poll.wit", include_str!("../wit/wasi-io/poll.wit")),
        ("streams.wit", include_str!("../wit/wasi-io/streams.wit")),
        ("world.wit", include_str!("../wit/wasi-io/world.wit")),
    ],
    license: Some(include_str!("../wit/wasi-io/LICENSE.md")),
    uses: &[],
};

/// Every package rk carries.
pub const PACKAGES: &[&Package] = &[&RK_PLC, &WASI_CLOCKS, &WASI_IO];

/// The package an interface belongs to, by the interface's name:
/// `wasi:clocks/wall-clock@0.2.6` is in `wasi:clocks`. `None` when rk
/// carries no such package.
pub fn package_of(interface: &str) -> Option<&'static Package> {
    let name = interface.split('/').next()?;
    PACKAGES
        .iter()
        .copied()
        .find(|package| package.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each package's files declare the name it is listed under, and the
    /// control interface's name is the one its file declares.
    #[test]
    fn each_package_declares_its_name() {
        for package in PACKAGES {
            for (file, text) in package.files {
                assert!(
                    text.lines()
                        .any(|line| line.starts_with(&format!("package {}@", package.name))),
                    "{}/{file} does not declare {}",
                    package.dir,
                    package.name
                );
            }
        }
        let (package, rest) = CONTROL_INTERFACE.split_once('/').unwrap();
        let (interface, version) = rest.split_once('@').unwrap();
        let text = RK_PLC.files[0].1;
        assert!(text.contains(&format!("package {package}@{version};")));
        assert!(text.contains(&format!("interface {interface} {{")));
    }
}
