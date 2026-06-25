module Ai
  # Re-points each workspace's document_analysis purpose at Anthropic (Claude) — the
  # vision model that reads full multi-page PDFs natively, replacing the OpenAI
  # page-1-rasterization path that only ever saw the first page (and misread amounts
  # off the JPEG). Idempotent and conservative:
  #
  #   • skips workspaces already on a usable Anthropic doc adapter
  #   • skips workspaces on managed "Campbooks AI" (that's a platform/billing/data-
  #     residency choice — not ours to flip from a backfill)
  #   • skips workspaces with no Anthropic key available
  #
  # The route itself goes through Ai::ProviderSetup#apply_documents, so the per-purpose
  # config and the (now-Claude) default model are wired exactly as the Settings UI does.
  class DocumentProviderRouter
    PROVIDER = "anthropic".freeze
    # A DEDICATED adapter for the document role. We deliberately do NOT reuse the
    # existing doc adapter in place (as the Settings flow does): that adapter is
    # often shared — e.g. it also drives compose_chat or is the workspace's only
    # embedding (OpenAI) adapter — so flipping its provider would break those. A
    # separate Claude adapter repoints only document_analysis and leaves the rest.
    ADAPTER_NAME = "Document AI provider (Claude)".freeze

    def self.run(dry_run: false, only_workspace_id: nil)
      new(dry_run: dry_run, only_workspace_id: only_workspace_id).run
    end

    def initialize(dry_run: false, only_workspace_id: nil)
      @dry_run = dry_run
      @only_workspace_id = only_workspace_id
    end

    def run
      workspaces.filter_map { |ws| consider(ws) }
    end

    private

    def workspaces
      scope = Workspace.all
      @only_workspace_id ? scope.where(id: @only_workspace_id) : scope
    end

    # The Anthropic key the routed adapter will use. On self-hosted the adapter
    # resolves the operator's own ENV key at call time (no stored key); on the cloud
    # a BYO adapter must carry the key, so we store it.
    def stored_key
      self_hosted? ? nil : ENV["ANTHROPIC_API_KEY"].presence
    end

    def consider(ws)
      current = ws.ai_configurations.find_by(purpose: "document_analysis")&.ai_adapter

      # Only re-point workspaces that already opted into document AI — never
      # auto-enable it (on a stored platform key) for a workspace that never set
      # a document provider up.
      return skip(ws, "no document provider configured") if current.nil?
      return skip(ws, "managed AI — not flipping")     if current&.managed?
      return skip(ws, "already on Anthropic")           if current&.provider == PROVIDER && current.usable?
      return skip(ws, "no ANTHROPIC_API_KEY available") if ENV["ANTHROPIC_API_KEY"].blank?

      from = current&.provider || "(unset)"
      route!(ws) unless @dry_run
      { workspace_id: ws.id, from: from, to: PROVIDER, model: AiConfiguration::DEFAULT_MODEL[PROVIDER] }
    end

    # Create/reuse the dedicated Claude doc adapter and point document_analysis at
    # it — touching nothing else the workspace's other purposes rely on.
    def route!(ws)
      adapter = ws.ai_adapters.find_or_initialize_by(name: ADAPTER_NAME)
      adapter.provider = PROVIDER
      adapter.managed = false
      adapter.enabled = true
      adapter.api_key = stored_key if stored_key.present?
      adapter.save!

      config = ws.ai_configurations.find_or_initialize_by(purpose: "document_analysis")
      config.ai_adapter = adapter
      config.enabled = true
      config.model = AiConfiguration::DEFAULT_MODEL[PROVIDER] unless AiConfiguration::MODELS[PROVIDER].include?(config.model)
      config.max_tokens ||= 1000
      config.temperature ||= 0.0
      config.save!
    end

    def skip(ws, reason)
      { workspace_id: ws.id, skipped: reason }
    end

    def self_hosted?
      Rails.application.config.self_hosted
    end
  end
end
