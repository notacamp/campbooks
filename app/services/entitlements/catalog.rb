module Entitlements
  # Loads config/plans.yml once, validates its structure, and exposes the
  # per-plan feature specifications. The single source of truth for what each
  # tier grants. Reloadable in tests / dev via .reload!.
  class Catalog
    class InvalidCatalog < StandardError; end

    PATH = Rails.root.join("config", "plans.yml")

    # Canonical default for new/blank workspaces. Mirrors plans.yml.
    DEFAULT_PLAN = "free".freeze

    class << self
      def instance
        @instance ||= new.tap(&:load!)
      end

      def reload!
        @instance = nil
        instance
      end

      # Plan names defined in the catalog — used by Workspace's inclusion validation.
      def plan_names
        instance.plan_names
      end
    end

    attr_reader :version

    def load!
      raw = YAML.safe_load_file(PATH, permitted_classes: [], aliases: false)
      raise InvalidCatalog, "config/plans.yml is empty" if raw.blank?

      @version = raw["version"]
      plans = raw["plans"]
      unless plans.is_a?(Hash) && plans.any?
        raise InvalidCatalog, "config/plans.yml has no `plans`"
      end

      @plans = plans.each_with_object({}) do |(name, features), acc|
        acc[name.to_s] = build_plan(name, features)
      end.freeze

      @feature_keys = @plans.values.flat_map(&:keys).uniq.freeze
      validate!
      self
    end

    # Returns { feature_key(Symbol) => FeatureSpecification } for the plan,
    # falling back to the default plan for an unknown name.
    def plan(name)
      @plans[name.to_s] || @plans.fetch(DEFAULT_PLAN)
    end

    def known_plan?(name)
      @plans.key?(name.to_s)
    end

    def plan_names
      @plans.keys
    end

    # Union of every feature key declared anywhere in the catalog.
    def feature_keys
      @feature_keys
    end

    private

    def build_plan(name, features)
      unless features.is_a?(Hash) && features.any?
        raise InvalidCatalog, "plan #{name} has no features"
      end

      features.each_with_object({}) do |(key, attrs), acc|
        acc[key.to_sym] = FeatureSpecification.from_hash(key, attrs)
      rescue KeyError => e
        raise InvalidCatalog, "plan #{name} feature #{key}: missing #{e.message}"
      end.freeze
    end

    def validate!
      @plans.each do |name, features|
        features.each do |key, spec|
          unless FeatureSpecification::TYPES.include?(spec.type)
            raise InvalidCatalog, "plan #{name} feature #{key}: unknown type #{spec.type.inspect}"
          end
          if spec.limit? && !spec.limit.nil? && !spec.limit.is_a?(Integer)
            raise InvalidCatalog, "plan #{name} feature #{key}: limit must be an integer or null"
          end
        end
      end
    end
  end
end
