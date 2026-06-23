# frozen_string_literal: true

# Entitlements / plan-catalog bootstrap.
#
# Loads + freezes the plan catalog (config/plans.yml) and caches the JSON Schema
# used to validate per-workspace entitlement_overrides. Runs inside to_prepare so
# it re-reads after a code reload in development; a malformed plans.yml raises here
# and fails boot (caught in CI). Mirrors config/initializers/registration.rb.
Rails.application.config.to_prepare do
  Entitlements::Catalog.reload!
  Rails.application.config.entitlements_schema = Entitlements::SchemaComposer.build
end
