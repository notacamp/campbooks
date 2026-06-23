module Ai
  # One-off, idempotent re-point of existing managed ("Campbooks AI") TEXT adapters
  # onto the current Platform::MANAGED_TEXT_PROVIDER (Mistral / Paris EU). New managed
  # setups already use it; this moves workspaces that opted into managed AI under the
  # old default (DeepSeek / China) so their email + chat content stops leaving the EU.
  #
  # Data-only (no schema migration). Run via `rake ai:repoint_managed_text` once
  # MISTRAL_API_KEY is set in the platform env. Re-running is a no-op.
  class ManagedTextRepointer
    def self.run
      target = Platform::MANAGED_TEXT_PROVIDER
      model  = AiConfiguration::DEFAULT_MODEL[target]
      valid  = AiConfiguration::MODELS[target] || []
      moved  = []

      Workspace.find_each do |workspace|
        adapter = workspace.ai_adapters.find_by(
          name: ProviderSetup::MANAGED_TEXT_ADAPTER_NAME, managed: true
        )
        next unless adapter

        from = adapter.provider
        repointed = adapter.provider != target
        adapter.update!(provider: target) if repointed

        # Managed adapters use the platform-chosen model. Reset any text purpose whose
        # stored model isn't valid for the target provider — whether left on the old
        # provider's model by the repoint above, or carried over by apply_managed
        # (which historically kept the existing model, stranding e.g. a Mistral adapter
        # on `deepseek-v4-pro` → the provider 400s). Idempotent: valid models untouched.
        fixed = workspace.ai_configurations
                         .where(purpose: AiConfiguration::TEXT_PURPOSES, ai_adapter: adapter)
                         .where.not(model: valid)
                         .update_all(model: model, updated_at: Time.current)

        moved << { workspace_id: workspace.id, from: from, to: target, models_fixed: fixed } if repointed || fixed.positive?
      end

      moved
    end
  end
end
