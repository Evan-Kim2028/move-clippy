/// Fixture package for the `witness_antipatterns` semantic lint.

module witness_antipatterns_pkg::witnesses {
    public struct UserWitness has drop, copy {}

    public struct StoredWitness has drop, store {}

    public fun create_user_witness(): UserWitness {
        UserWitness {}
    }
}
