use crate::opamp::proto::{PackageStatuses, PackageType};

/// PackagesSyncer can be used by the Agent to initiate syncing a package from the Server.
/// The PackagesSyncer instance knows the right context: the particular OpAMPClient and
/// the particular PackageAvailable message the OnPackageAvailable callback was called for.
trait Syncer {
    type Error: std::error::Error + Send + Sync;

    // Sync the available package from the Server to the Agent.
    // The Agent must supply an PackagesStateProvider in StartSettings to let the Sync
    // function know what is available locally, what data needs to be synced and how the
    // data can be stored locally.
    // Sync typically returns immediately and continues working in the background,
    // downloading the packages and applying the changes to the local state.
    // Sync should be called once only.
    fn sync(&self) -> Result<(), Self::Error>;

    // Done returns a channel which is readable when the [`sync`] is complete.
    // fn done(&self) <-chan struct{} // FIXME this is from Go
}

#[allow(dead_code)] // FIXME: remove when actually using!
pub struct Package {
    package_type: PackageType,
    hash: Vec<u8>,
    version: String,
}

type PackageState = Option<Package>;

/// A trait that is used by a [`PackageSyncer`]'s [`sync()`] to
/// query and update the Agent's local state of packages.
/// It is recommended that the local state is stored persistently so that after
/// Agent restarts full state syncing is not required.
pub trait PackagesStateProvider {
    type Error: std::error::Error + Send + Sync;

    /// Returns the hash of all packages previously set via the [`set_all_packages_hash()`] method.
    fn all_packages_hash(&self) -> Result<Vec<u8>, Self::Error>;

    /// This must remember the [`all_packages_hash()`]. Must be returned
    /// later when [`all_packages_hash()`] is called. [`set_all_packages_hash`] is called after all
    /// package updates complete successfully.
    fn set_all_packages_hash(&mut self, hash: &[u8]) -> Result<(), Self::Error>;

    /// Returns the names of all packages that exist in the Agent's local storage.
    fn packages(&self) -> Result<Vec<String>, Self::Error>;

    /// Returns the state of a local package. packageName is one of the names
    /// that were returned by [`packages()`].
    /// Returns (PackageState{Exists:false},nil) if package does not exist locally.
    fn package_state(&self, package_name: String) -> Result<&Package, Self::Error>;

    /// [`set_package_state`] must remember the state for the specified package. Must be returned
    /// later when [`package_state`]  is called. [`set_package_state`] is called after `[update_content`]
    /// call completes successfully.
    /// The [`state.Type`] must be equal to the current [`Type`] of the package otherwise
    /// the call may fail with an error.
    fn set_package_state(
        &self,
        package_name: String,
        state: PackageState,
    ) -> Result<(), Self::Error>;

    /// Creates the package locally. If the package existed must return an error.
    /// If the package did not exist its hash should be set to nil.
    fn create_package(
        &self,
        package_name: String,
        package_type: PackageType,
    ) -> Result<(), Self::Error>;

    /// Returns the content hash of the package file that exists locally.
    /// Returns (nil,nil) if package or package file is not found.
    fn file_content_hash(&self, package_name: String) -> Result<&[u8], Self::Error>;

    /// [`update_content`] must create or update the package content file. The entire content
    /// of the file must be replaced by the data. The data must be read until
    /// it returns an EOF. If reading from data fails [`update_content`] must abort and return
    /// an error.
    /// Content hash must be updated if the data is updated without failure.
    /// The function must cancel and return an error if the context is cancelled.
    // FIXME: is the above line needed?
    fn update_content(
        &self,
        package_name: String,
        data: &[u8],
        content_hash: &[u8],
    ) -> Result<(), Self::Error>;

    /// deletes the package from the Agent's local storage.
    fn delete_package(&self, package_name: String) -> Result<(), Self::Error>;

    /// Returns the value previously set via [`set_last_reported_statuses`].
    fn last_reported_statuses(&self) -> Result<PackageStatuses, Self::Error>;

    /// Saves the statuses in the local state. This is called
    /// periodically during syncing process to save the most recent statuses.
    fn set_last_reported_statuses(&self, statuses: &PackageStatuses) -> Result<(), Self::Error>;
}
