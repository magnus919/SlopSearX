param([string]$Inventory, [switch]$IgnoredOnly)
$ErrorActionPreference = 'Stop'
$lines = @(Get-Content -LiteralPath $Inventory)
$store = 'installed_platform_verifier_rejects_private_ca_then_accepts_exact_system_store_and_still_checks_hostname'
if ($IgnoredOnly) { $expected = @($store) } else {
    $expected = @(
      'native_distribution_installs_and_invokes_exact_target_without_backend_runtime',
      'installed_additive_ca_refuses_missing_malformed_bundle_and_wrong_hostname',
      'installed_tls_connect_proxy_keeps_proxy_credentials_out_of_origin_and_bearer_out_of_connect',
      'installed_tls_cross_origin_redirect_is_rejected_without_destination_request', $store,
      'actual_native_static_process_preserves_signature_schema_projection_error_channels_and_eof_cleanup',
      'actual_native_oauth_process_challenge_registration_pkce_callback_secure_persistence_and_cleanup',
      'actual_native_startup_failure_redacts_configuration_and_never_publishes',
      'actual_native_two_instances_keep_remote_sessions_and_catalogs_independent',
      'actual_native_stdio_eof_cancels_inflight_rpc_before_owned_shutdown',
      'actual_native_oauth_non200_refresh_falls_back_to_authorization_without_second_registration',
      'actual_native_static_userinfo_basic_precedes_bearer_and_fragment_never_reaches_http',
      'actual_native_oauth_status_matrix_preserves_registration_code_200_201_and_rejects_202_without_token_persistence',
      'actual_native_catalog_validation_diagnostics_are_source_exact_and_never_dispatch'
    )
    if ($IsWindows) {
      $expected += @(
        'windows_numeric_source_ports_use_exact_proactor_mask_without_lookup_child',
        'windows_dropped_lookup_joins_original_helper_job_without_input_eof',
        'windows_dropped_close_future_retains_registered_original_owner_for_retry',
        'windows_protocol_failure_and_inherited_descendant_do_not_escape_job_retirement',
        'windows_actual_input_idle_eof_backpressure_and_dropped_close_join_original_job',
        'windows_token_storage_confidentiality_atomic_replacement_and_collision_ownership',
        'windows_token_reparse_hardlink_and_untrusted_writer_parent_are_rejected',
        'windows_installed_digest_reparse_and_writer_acl_fail_before_helper_admission',
        'windows_atomic_job_assignment_failure_never_runs_child_and_valid_control_retires',
        'windows_abrupt_original_parent_death_closes_last_job_handle_and_retires_entered_child',
        'windows_held_confidential_read_atomic_replace_and_parent_rename_have_coherent_kernel_outcomes'
      )
    } else {
      $expected += @(
        'actual_native_sigterm_during_initialization_joins_http_owner',
        'actual_native_oauth_denied_delete_on_eof_and_sigterm_never_reauthorizes_or_refreshes_and_retires_owners',
        'actual_native_held_oauth_request_cancel_eof_sigterm_do_not_reenter_auth_or_deadlock_and_retire_owners'
      )
    }
}
foreach ($name in $expected) {
    if (@($lines | Where-Object { $_ -ceq "${name}: test" }).Count -ne 1) { throw "Missing or duplicated compiled test: $name" }
}
if (@($lines | Where-Object { $_ -match 'actual_native_current_t06_eleven_tool_catalog_and_discovery_match_direct_http: test' }).Count -ne 0) { throw 'Server-only fixture entered client-only inventory' }
if ($IgnoredOnly -and @($lines | Where-Object { $_ -match ': test$' }).Count -ne 1) { throw 'Hosted trust-store exact inventory expanded unexpectedly' }
Write-Host "Verified $($expected.Count) explicit native test identities; this is inventory, not execution evidence."
