/// A _permissioned signer_ consists of a pair of the original signer and a generated
/// signer which is used store information about associated permissions.
///
/// A permissioned signer behaves compatible with the original signer as it comes to `move_to`, `address_of`, and
/// existing basic signer functionality. However, the permissions can be queried to assert additional
/// restrictions on the use of the signer.
///
/// A client which is interested in restricting access granted via a signer can create a permissioned signer
/// and pass on to other existing code without changes to existing APIs. Core functions in the framework, for
/// example account functions, can then assert availability of permissions, effectively restricting
/// existing code in a compatible way.
///
/// After introducing the core functionality, examples are provided for withdraw limit on accounts, and
/// for blind signing.
module permissioned_signer::permissioned_signer {
    use std::signer::address_of;
    use aptos_framework::transaction_context::generate_auid_address;

    struct PermissionedSigner has store {
        master_addr: address,
        permission_addr: address,
    }

    public fun create_permissioned_signer(master: &signer): PermissionedSigner {
        assert!(!is_permissioned_signer(master));
        PermissionedSigner {
            master_addr: address_of(master),
            permission_addr: generate_auid_address(),
        }
    }

    // =====================================================================================================
    // Native Functions

    /// Creates a permissioned signer from an existing universal signer. The function aborts if the
    /// given signer is already a permissioned signer.
    ///
    /// The implementation of this function requires to extend the value representation for signers in the VM.
    ///
    /// Check whether this is a permissioned signer.
    public(package) native fun is_permissioned_signer(s: &signer): bool;
    /// Return the signer used for storing permissions. Aborts if not a permissioned signer.
    public(package) native fun permission_signer(permissioned: &signer): &signer;

    public native fun signer_from_permissioned(p: &PermissionedSigner): signer;
}
