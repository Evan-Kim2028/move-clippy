// Fixture package for full-mode preview gating.
//
// Expected behavior:
// - `missing_access_control` (Preview) should only be reported when preview=true.

module preview_gating_pkg::balance {
    // Stub: matches privileged sink name `balance::increase_supply`.
    public fun increase_supply() {}
}

module preview_gating_pkg::state {
    use preview_gating_pkg::balance;

    public struct State has store {
        v: u64,
    }

    // Performs a privileged sink but has no auth-token parameter.
    // This should trigger `missing_access_control` when preview is enabled.
    public fun unsafe_admin_action(s: &mut State) {
        balance::increase_supply();
        s.v = s.v + 1;
    }
}
