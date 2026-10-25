//! Private Windows OS boundary. No caller-controlled commands or raw handles.
//! Application crates keep unsafe_code=forbid. See SAFETY.md for the FFI scope.
#![cfg(windows)]
mod dns;
mod process;
mod trust;
pub use dns::{LookupError, lookup, numeric_host, source_connect_port};
pub use process::{ChildPipes, ExitHandle, ExitObservation, JobCancel, NativeChild, Retirement};
pub use trust::{
    FileDigest, ImageRole, TrustedImage, build_file_digest, check_private_directory,
    create_private_directory, create_private_file, current_input, package_file_digest,
    package_lock, read_package_file,
};

/// Exact CPython _overlapped.parse_address `H` conversion at the Windows
/// proactor connect stage. This must never be used as URL-parser validation.
#[cfg(test)]
mod tests {
    use super::source_connect_port;
    #[test]
    fn windows_proactor_preserves_source_mask_at_actual_connect_boundary() {
        assert_eq!(source_connect_port("65536").unwrap(), 0);
        assert_eq!(source_connect_port("65537").unwrap(), 1);
        assert_eq!(source_connect_port("-1").unwrap(), 65535);
        assert_eq!(
            source_connect_port("170141183460469231731687303715884105728").unwrap(),
            0
        );
        assert_eq!(
            source_connect_port(&format!("1{}", "0".repeat(4299))).unwrap(),
            0
        );
        for bad in ["", "+1", "01", "-0", "1.0", " 1", "1e3"] {
            assert!(source_connect_port(bad).is_err());
        }
        assert!(source_connect_port(&"9".repeat(4301)).is_err());
    }
}

#[cfg(feature = "fixtures")]
pub use process::FixtureInput;
