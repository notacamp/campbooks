module Ai
  # One-off, idempotent re-point of existing managed ("Campbooks AI") TEXT adapters
  # onto the current Platform::MANAGED_TEXT_PROVIDER (Mistral / Paris EU). New managed
  # setups already use it; this moves workspaces that opted into managed AI under the
  # old default (DeepSeek / China) so their email + chat content stops leaving the EU.
  #
  # Also syncs the AI gateway endpoint (AI_MANAGED_ENDPOINT) onto all existing managed
  # text adapters so they pick up a newly-configured (or removed) gateway without
  # having to re-provision each workspace. Setting the ENV and running this task once
  # is enough; new managed workspaces pick the gateway up via apply_managed.
  #
  # Data-only (no schema migration). Run via `rake ai:repoint_managed_text` once
  # MISTRAL_API_KEY (or AI_MANAGED_GATEWAY_KEY) is set in the platform env.
  # Re-running is a no-op when nothing has changed.
  class ManagedTextRepointer
    def self.run
      target           = Platform::MANAGED_TEXT_PROVIDER
      model            = AiConfiguration::DEFAULT_MODEL[target]
      valid            = AiConfiguration::MODELS[target] || []
      gateway_endpoint = Platform.managed_gateway_endpoint
      moved            = []

      Workspace.find_each do |workspace|
        adapter = workspace.ai_adapters.find_by(
          name: ProviderSetup::MANAGED_TEXT_ADAPTER_NAME, managed: true
        )
        next unless adapter

        from         = adapter.provider
        repointed    = adapter.provider != target
        endpoint_changed = adapter.endpoint_url != gateway_endpoint

        if repointed || endpoint_changed
          adapter.update!(provider: target, endpoint_url: gateway_endpoint)
        end

        # Managed adapters use the platform-chosen model. Reset any text purpose whose
        # stored model isn't valid for the target provider — whether left on the old
        # provider's model by the repoint above, or carried over by apply_managed
        # (which historically kept the existing model, stranding e.g. a Mistral adapter
        # on `deepseek-v4-pro` → the provider 400s). Idempotent: valid models untouched.
        fixed = workspace.ai_configurations
                         .where(purpose: AiConfiguration::TEXT_PURPOSES, ai_adapter: adapter)
                         .where.not(model: valid)
                         .update_all(model: model, updated_at: Time.current)

        if repointed || endpoint_changed || fixed.positive?
          moved << { workspace_id: workspace.id, from: from, to: target,
                     models_fixed: fixed, endpoint_changed: endpoint_changed }
        end
      end

      moved
    end
  end
end
