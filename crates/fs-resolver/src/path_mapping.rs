use std::path::Component;

use crate::WindowsApplicationDataPath;
use crate::android_paths::AndroidPath;
use crate::error::{Error, Result};
use crate::ios_paths::IosPath;
use crate::linux_paths::LinuxPath;
use crate::mac_paths::MacPath;
use crate::windows_paths::Win32Path;

/// Per-platform path mapping: a platform path enum plus an optional relative suffix.
///
/// Resolved in-process via [`PathResolver::resolve_mapping`](crate::PathResolver::resolve_mapping).
/// Not serialized over IPC — only individual path enums cross the plugin boundary.
///
/// When `relative_path` is set, the resolver joins it to the resolved platform path.
/// The relative path must contain only normal path segments (no absolute paths, `.`, `..`,
/// or empty segments).
#[derive(Debug)]
pub struct PlatformMapping<T: PlatformPath> {
   pub platform_path: T,
   pub relative_path: Option<String>,
}

/// In a packaged app, either an ApplicationDataPath or a Win32Path can be used.
/// With this in mind, we need to allow a CrossPlatformMapping to support both of these paths
/// when defining the mapping used for a packaged app.
#[derive(Debug)]
pub enum WinPackagedPathMapping {
   WindowsApplicationDataPath(PlatformMapping<WindowsApplicationDataPath>),
   Win32Path(PlatformMapping<Win32Path>),
}

/// Cross-platform path definition with optional per-OS mappings.
///
/// Parallel to the TypeScript `CrossPlatformMapping` type. Each platform field holds a
/// [`PlatformMapping`] for that OS. At resolution time, only the mapping for the current
/// platform is used. Not sent across IPC.
#[derive(Debug)]
pub struct CrossPlatformMapping {
   pub android: Option<PlatformMapping<AndroidPath>>,
   pub ios: Option<PlatformMapping<IosPath>>,
   pub macos: Option<PlatformMapping<MacPath>>,
   pub linux: Option<PlatformMapping<LinuxPath>>,
   pub win32: Option<PlatformMapping<Win32Path>>,
   pub win_packaged: Option<WinPackagedPathMapping>,
}

/// Validates that `relative_path` contains only normal path segments.
///
/// Rejects absolute paths, `.`, `..`, and empty segments so `PathBuf::join` cannot
/// silently replace the resolved base path.
pub fn validate_relative_path(relative_path: &str) -> Result<()> {
   if relative_path.is_empty() {
      return Err(Error::InvalidPath(
         "Relative path must not be empty".to_string(),
      ));
   }

   for segment in relative_path.split(['/', '\\']) {
      if segment.is_empty() || segment == "." || segment == ".." {
         return Err(Error::InvalidPath(format!(
            "Relative path must contain only normal path segments: {relative_path}"
         )));
      }
   }

   for component in std::path::Path::new(relative_path).components() {
      if !matches!(component, Component::Normal(_)) {
         return Err(Error::InvalidPath(format!(
            "Relative path must contain only normal path segments: {relative_path}"
         )));
      }
   }

   Ok(())
}

/// Validates that `bundle_identifier` is safe to append as a single path segment.
///
/// Rejects empty identifiers, path separators, `.`, `..`, and absolute paths so
/// `PathBuf::join` cannot silently return the shared base directory or escape it.
pub fn validate_bundle_identifier(bundle_identifier: &str) -> Result<()> {
   if bundle_identifier.contains('/') || bundle_identifier.contains('\\') {
      return Err(Error::InvalidPath(format!(
         "Bundle identifier must be a single safe path segment: {bundle_identifier}"
      )));
   }

   validate_relative_path(bundle_identifier).map_err(|_| {
      Error::InvalidPath(format!(
         "Bundle identifier must be a single safe path segment: {bundle_identifier}"
      ))
   })
}

// This is a marker to ensure that PlatformMapping can only be instantiated with a type
// that is defined in this module.
pub trait PlatformPath: private::Sealed {}
mod private {
   pub trait Sealed {}
}

impl private::Sealed for AndroidPath {}
impl private::Sealed for IosPath {}
impl private::Sealed for MacPath {}
impl private::Sealed for LinuxPath {}
impl private::Sealed for Win32Path {}
impl private::Sealed for WindowsApplicationDataPath {}

impl PlatformPath for AndroidPath {}
impl PlatformPath for IosPath {}
impl PlatformPath for MacPath {}
impl PlatformPath for LinuxPath {}
impl PlatformPath for Win32Path {}
impl PlatformPath for WindowsApplicationDataPath {}

#[cfg(test)]
mod tests {
   use super::*;
   use rstest::rstest;

   #[rstest]
   #[case::data("data")]
   #[case::the_android_path("the/android/path")]
   #[case::nested_deep_path("nested/deep/path")]
   fn validate_relative_path_accepts_normal_segments(#[case] segment: &str) {
      validate_relative_path(segment).unwrap();
   }

   #[test]
   fn validate_relative_path_rejects_empty() {
      assert_eq!(
         validate_relative_path(""),
         Err(Error::InvalidPath(
            "Relative path must not be empty".to_string()
         ))
      );
   }

   #[rstest]
   #[case::current_dir(".")]
   #[case::parent_dir("..")]
   #[case::absolute_path("/absolute")]
   #[case::trailing_parent_dir("foo/..")]
   #[case::leading_parent_dir("../foo")]
   #[case::trailing_current_dir("foo/.")]
   #[case::leading_current_dir("./foo")]
   #[case::consecutive_slashes("foo//bar")]
   fn validate_relative_path_rejects_invalid_segments(#[case] segment: &str) {
      assert_eq!(
         validate_relative_path(segment),
         Err(Error::InvalidPath(format!(
            "Relative path must contain only normal path segments: {segment}"
         )))
      );
   }

   #[rstest]
   #[case::com_example_app("com.example.app")]
   fn validate_bundle_identifier_accepts_valid_ids(#[case] id: &str) {
      validate_bundle_identifier(id).unwrap();
   }

   #[rstest]
   #[case::empty("")]
   #[case::parent_dir("..")]
   #[case::absolute_path("/absolute")]
   #[case::path_separator("com/example")]
   fn validate_bundle_identifier_rejects_invalid_ids(#[case] id: &str) {
      assert_eq!(
         validate_bundle_identifier(id),
         Err(Error::InvalidPath(format!(
            "Bundle identifier must be a single safe path segment: {id}"
         )))
      );
   }
}
