module Entitlements
  # One feature's entry within a single plan, as loaded from config/plans.yml.
  # Immutable value object; the Catalog builds these once at boot. The Resolver
  # deep-merges a workspace's overrides onto #to_h to produce effective values.
  class FeatureSpecification
    TYPES = %i[flag limit config].freeze

    attr_reader :key, :type, :config

    def initialize(key:, type:, allowed: true, enabled: true, limit: nil, config: {})
      @key     = key.to_sym
      @type    = type.to_sym
      @allowed = !!allowed
      @enabled = !!enabled
      @limit   = limit
      @config  = (config || {}).deep_symbolize_keys.freeze
      freeze
    end

    def allowed? = @allowed
    def enabled? = @enabled

    # GoVocal's feature_activated? rule.
    def active? = allowed? && enabled?

    # Integer cap, or nil for unlimited. Only meaningful for :limit features.
    def limit = @limit

    def flag?   = type == :flag
    def limit?  = type == :limit
    def config? = type == :config

    # Build from a raw plans.yml hash (string keys). Raises KeyError if `type`
    # is missing (caught + reported by the Catalog with the plan/feature name).
    def self.from_hash(key, hash)
      h = hash.to_h.symbolize_keys
      new(
        key:     key,
        type:    h.fetch(:type),
        allowed: h.fetch(:allowed, true),
        enabled: h.fetch(:enabled, true),
        limit:   h.key?(:limit) ? h[:limit] : nil,
        config:  h[:config] || {}
      )
    end

    def to_h
      { type: type, allowed: allowed?, enabled: enabled?, limit: limit, config: config }
    end
  end
end
