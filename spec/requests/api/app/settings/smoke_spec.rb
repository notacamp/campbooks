# frozen_string_literal: true

require "rails_helper"

# Smoke test every settings/notifications collection GET on a fresh (empty)
# workspace: none may 500. This is the net the per-controller specs miss — the
# "passes specs but breaks live" class (namespace collisions, nil handling,
# rescue-swallowed errors) that only surfaces when the endpoint actually runs.
RSpec.describe "API app settings — no endpoint 500s", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace, role: :admin) }
  let(:headers)   { api_app_headers(user) }

  SMOKE_PATHS = %w[
    /api/app/account
    /api/app/settings/workspace
    /api/app/settings/plan
    /api/app/settings/members
    /api/app/settings/ai
    /api/app/settings/ai_adapters
    /api/app/settings/ai_prompts
    /api/app/settings/data_privacy
    /api/app/settings/integrations
    /api/app/settings/integrations/notion
    /api/app/settings/integrations/calendars
    /api/app/settings/integrations/connections
    /api/app/settings/memory
    /api/app/inbox_settings/tags
    /api/app/inbox_settings/rules
    /api/app/inbox_settings/signatures
    /api/app/inbox_settings/document_types
    /api/app/inbox_settings/tag_groups
    /api/app/inbox_settings/filtering
    /api/app/notifications
  ].freeze

  SMOKE_PATHS.each do |path|
    it "GET #{path} does not 500" do
      get path, headers: headers

      expect(response.status).to(be < 500,
        "#{path} returned #{response.status}: #{response.body[0, 300]}")
    end
  end
end
