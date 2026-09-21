# frozen_string_literal: true

require "rails_helper"

# Whole-API smoke: every parameterless /api/app GET (across all surfaces) must not
# 500 on a fresh, empty workspace. Complements the per-surface specs, which mostly
# run against POPULATED data — this catches the empty-state / routing / nil-handling
# 500s that only appear when an endpoint runs with nothing there (the class that hid
# the Inbox StandingsRefreshJob 500 and the 7 singular-resource→plural-controller
# settings breaks). Settings + notifications GETs have their own smoke_spec.
#
# A 400/401/403/422 is fine (missing param, gate, empty) — only a 5xx is a bug.
RSpec.describe "API app — no screen GET 500s (empty workspace)", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace, role: :admin) }
  let(:headers)   { api_app_headers(user) }

  SCREEN_GETS = %w[
    /api/app/me
    /api/app/now
    /api/app/today
    /api/app/time
    /api/app/money
    /api/app/money/loans
    /api/app/money/export
    /api/app/reconciliations
    /api/app/calendar
    /api/app/event_types
    /api/app/people
    /api/app/people/streams
    /api/app/organizations
    /api/app/activity
    /api/app/paper
    /api/app/files
    /api/app/files/public_links/picker
    /api/app/document_skim
    /api/app/document_skim/tray
    /api/app/documents/merge
    /api/app/email_messages
    /api/app/email_messages/search
    /api/app/email_threads
    /api/app/email_accounts
    /api/app/email_skim
    /api/app/email_skim/tray
    /api/app/drafts
    /api/app/drafts/new
    /api/app/compose/prefill
    /api/app/scout/overlay
    /api/app/scout/threads
    /api/app/scout/threads/new
    /api/app/scout/unread
    /api/app/search
    /api/app/onboarding
    /api/app/onboarding/first_sync_status
  ].freeze

  SCREEN_GETS.each do |path|
    it "GET #{path} does not 500" do
      get path, headers: headers

      expect(response.status).to(be < 500,
        "#{path} returned #{response.status}: #{response.body[0, 300]}")
    end
  end
end
