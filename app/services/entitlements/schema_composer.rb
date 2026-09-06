module Entitlements
  # Builds the JSON Schema (Draft 2020-12) that validates a workspace's
  # entitlement_overrides jsonb, and runs validation via json_schemer.
  #
  # Overrides shape (every key optional; only known feature keys allowed):
  #   { "<feature_key>" => { "allowed"=>bool, "enabled"=>bool,
  #                          "limit"=>int|null, "config"=>{...} } }
  module SchemaComposer
    module_function

    def build(catalog = Catalog.instance)
      feature_props = catalog.feature_keys.index_with { feature_override_schema }
                             .transform_keys(&:to_s)

      {
        "$schema" => "https://json-schema.org/draft/2020-12/schema",
        "type" => "object",
        "additionalProperties" => false,
        "properties" => feature_props
      }
    end

    def feature_override_schema
      {
        "type" => "object",
        "additionalProperties" => false,
        "properties" => {
          "allowed" => { "type" => "boolean" },
          "enabled" => { "type" => "boolean" },
          "limit"   => { "type" => %w[integer null] },
          "config"  => { "type" => "object" }
        }
      }
    end

    # Returns an array of human-readable error strings ([] when valid). `schema` is
    # a positional optional (not a kwarg) so a bare string-keyed hash literal can be
    # passed without Ruby consuming it as keywords.
    # Features that left the catalog. An override for one of these is a leftover from
    # when the feature was paid (a data migration strips them, but a restored dump can
    # bring one back); it is ignored rather than failing the whole workspace record.
    RETIRED_FEATURES = %w[tasks].freeze

    def validate_overrides(overrides, schema = nil)
      return [] if overrides.blank?

      live = overrides.deep_stringify_keys.except(*RETIRED_FEATURES)
      return [] if live.blank?

      schema ||= Rails.application.config.try(:entitlements_schema) || build
      schemer = JSONSchemer.schema(schema)
      schemer.validate(live).map { |err| message_for(err) }
    end

    def message_for(err)
      return err["error"] if err["error"].present?

      pointer = err["data_pointer"].presence || "(root)"
      type = err["type"].presence || "invalid"
      "#{pointer}: #{type}"
    end
  end
end
