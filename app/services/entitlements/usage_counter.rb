module Entitlements
  # Maps a feature key to a live usage count for a workspace. This is the metering
  # seam: in-scope (stock) features count straight from the DB; deferred volume
  # features map to nil until the metering layer is built — then swap the nil for a
  # counter lookup here and no resolver/controller call site changes.
  module UsageCounter
    COUNTERS = {
      email_accounts: ->(ws) { ws.email_accounts.active.count },
      workflows:      ->(ws) { ws.workflows.count },
      pipelines:      ->(ws) { ws.pipelines.count },

      # Deferred (no per-period metering yet). Phase 2 swaps in a real counter.
      emails_synced:       nil,
      workflow_executions: nil,
      notifications:       nil
    }.freeze

    module_function

    # Integer current usage, or nil when the feature isn't metered.
    def count(key, workspace)
      return nil if workspace.nil?

      fn = COUNTERS[key.to_sym]
      fn&.call(workspace)
    end

    def metered?(key)
      !COUNTERS[key.to_sym].nil?
    end
  end
end
