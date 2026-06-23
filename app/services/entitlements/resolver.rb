module Entitlements
  # The effective entitlements for one workspace: the plan's catalog entry,
  # deep-merged with that workspace's per-workspace overrides. Cloud only —
  # self-hosted workspaces get a NullResolver instead (Workspace#entitlements).
  class Resolver
    attr_reader :workspace

    def initialize(workspace, catalog: Catalog.instance)
      @workspace = workspace
      @catalog = catalog
    end

    # The resolved plan name, falling back to the default for blank/unknown.
    def plan_name
      name = workspace.plan.presence || Catalog::DEFAULT_PLAN
      @catalog.known_plan?(name) ? name : Catalog::DEFAULT_PLAN
    end

    # allowed && enabled for the feature. False for keys not in the plan.
    def feature?(key)
      spec = resolved(key)
      return false if spec.nil?

      !!spec[:allowed] && !!spec[:enabled]
    end

    # Integer cap, or nil for unlimited / non-limit features.
    def limit(key)
      resolved(key)&.dig(:limit)
    end

    # Config value: the whole hash, or a single sub-key.
    def config(key, subkey = nil)
      cfg = resolved(key)&.dig(:config) || {}
      subkey ? cfg[subkey.to_sym] : cfg
    end

    # Live usage count (nil when the feature isn't metered yet).
    def usage(key)
      UsageCounter.count(key, workspace)
    end

    # limit - usage (floored at 0), or nil when unlimited / not metered.
    def remaining(key)
      cap = limit(key)
      return nil if cap.nil?

      [ cap - (usage(key) || 0), 0 ].max
    end

    # True when the workspace is currently above a cap (e.g. after a downgrade).
    def over_cap?(key)
      cap = limit(key)
      return false if cap.nil?

      used = usage(key)
      !used.nil? && used > cap
    end

    # :ok | :not_allowed | :not_enabled | :over_limit
    # Unknown keys are never gated (returns :ok) so callers can guard liberally.
    def allow?(key)
      spec = resolved(key)
      return :ok if spec.nil?
      return :not_allowed unless spec[:allowed]
      return :not_enabled unless spec[:enabled]

      if spec[:type] == :limit && !spec[:limit].nil?
        used = usage(key)
        return :over_limit if !used.nil? && used >= spec[:limit]
      end
      :ok
    end

    # Raises a typed error unless allow? is :ok. Returns true otherwise.
    def allow!(key)
      case allow?(key)
      when :over_limit
        raise LimitExceeded.new(key: key, limit: limit(key))
      when :not_allowed, :not_enabled
        raise FeatureNotAllowed.new(key: key)
      end
      true
    end

    # Per-feature resolved view for the Settings → Plan page.
    def summary
      specs = @catalog.plan(plan_name)
      specs.keys.index_with do |key|
        {
          type:      specs[key].type,
          active:    feature?(key),
          limit:     limit(key),
          usage:     usage(key),
          remaining: remaining(key),
          over_cap:  over_cap?(key),
          config:    config(key)
        }
      end
    end

    private

    # Merged effective spec hash for a feature (plan base ⊕ workspace override),
    # or nil if the plan doesn't define it. `type` is structural and never
    # overridable.
    def resolved(key)
      key = key.to_sym
      spec = @catalog.plan(plan_name)[key]
      return nil if spec.nil?

      base = spec.to_h
      override = overrides[key]
      merged = override.is_a?(Hash) ? base.deep_merge(override) : base
      merged[:type] = spec.type
      merged
    end

    def overrides
      @overrides ||= (workspace.entitlement_overrides || {}).deep_symbolize_keys
    end
  end
end
