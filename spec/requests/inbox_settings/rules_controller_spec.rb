require "rails_helper"

RSpec.describe "InboxSettings::RulesController", type: :request do
  before do
    sign_in_as(user)
  end

  let(:workspace) do
    Workspace.create!(name: "Rules WS", slug: "rules-ws-#{SecureRandom.hex(4)}")
  end

  let(:user) do
    workspace.users.create!(
      name: "Ruler",
      email_address: "ruler-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  let(:tag) { workspace.tags.create!(name: "Finance", color: "#2563eb") }

  def build_rule(overrides = {})
    workspace.email_rules.create!({
      name: "Test rule",
      criteria: { "from" => [ "@example.com" ] },
      archive: true,
      mark_read: false,
      enabled: true,
      created_by: user
    }.merge(overrides))
  end

  # --- index ---

  it "GET index renders the panel" do
    get inbox_settings_rules_path
    expect(response).to have_http_status(:ok)
  end

  it "GET index lists rules for the current workspace" do
    rule = build_rule(name: "My rule")
    get inbox_settings_rules_path
    expect(response.body).to include("My rule")
  end

  # --- new / create ---

  it "GET new renders the form" do
    get new_inbox_settings_rule_path
    expect(response).to have_http_status(:ok)
  end

  it "POST create with valid params creates the rule" do
    expect {
      post inbox_settings_rules_path,
           params: { email_rule: { name: "My rule", criteria: { from: "@stripe.com" }, archive: "1" } },
           as: :turbo_stream
    }.to change(EmailRule, :count).by(1)

    expect(response).to have_http_status(:ok)
    rule = EmailRule.last
    expect(rule.name).to eq("My rule")
    expect(rule.archive).to be(true)
    expect(rule.workspace).to eq(workspace)
    expect(rule.created_by).to eq(user)
  end

  it "POST create with invalid params re-renders the form without persisting" do
    expect {
      post inbox_settings_rules_path,
           params: { email_rule: { name: "", criteria: {}, archive: "0" } },
           as: :turbo_stream
    }.not_to change(EmailRule, :count)

    expect(response).to have_http_status(:unprocessable_entity)
  end

  it "POST create assigns tag_ids from params" do
    post inbox_settings_rules_path,
         params: { email_rule: {
           name: "Tagged rule",
           criteria: { from: "@example.com" },
           tag_ids: [ tag.id ]
         } },
         as: :turbo_stream

    rule = EmailRule.last
    expect(rule.tags).to include(tag)
  end

  it "POST create with run_on_existing enqueues EmailRuleRunJob" do
    expect {
      post inbox_settings_rules_path,
           params: { email_rule: {
             name: "Run now",
             criteria: { from: "@example.com" },
             archive: "1",
             run_on_existing: "1"
           } },
           as: :turbo_stream
    }.to have_enqueued_job(EmailRuleRunJob)
  end

  # --- edit / update ---

  it "GET edit renders the form with the rule" do
    rule = build_rule
    get edit_inbox_settings_rule_path(rule)
    expect(response).to have_http_status(:ok)
    expect(response.body).to include(rule.name)
  end

  it "PATCH update with valid params persists the changes" do
    rule = build_rule(name: "Old name")
    patch inbox_settings_rule_path(rule),
          params: { email_rule: { name: "New name", criteria: { subject: "invoice" } } },
          as: :turbo_stream

    expect(response).to have_http_status(:ok)
    expect(rule.reload.name).to eq("New name")
  end

  it "PATCH update from another workspace 404s" do
    other_ws = Workspace.create!(name: "Other", slug: "other-#{SecureRandom.hex(4)}")
    other_rule = other_ws.email_rules.create!(
      name: "Theirs",
      criteria: { "from" => [ "@evil.com" ] },
      archive: true,
      mark_read: false,
      enabled: true
    )

    patch inbox_settings_rule_path(other_rule),
          params: { email_rule: { name: "Hijacked" } },
          as: :turbo_stream
    expect(response).to have_http_status(:not_found)
  end

  # --- destroy ---

  it "DELETE destroy removes the rule" do
    rule = build_rule
    expect {
      delete inbox_settings_rule_path(rule), as: :turbo_stream
    }.to change(EmailRule, :count).by(-1)

    expect(response).to have_http_status(:ok)
  end

  # --- toggle ---

  it "PATCH toggle flips the enabled flag" do
    rule = build_rule(enabled: true)
    patch toggle_inbox_settings_rule_path(rule), as: :turbo_stream

    expect(response).to have_http_status(:ok)
    expect(rule.reload.enabled).to be(false)
  end

  it "PATCH toggle from disabled enables the rule" do
    rule = build_rule(enabled: false)
    patch toggle_inbox_settings_rule_path(rule), as: :turbo_stream

    expect(rule.reload.enabled).to be(true)
  end

  # --- run ---

  it "POST run creates a queued EmailRuleRun and enqueues EmailRuleRunJob" do
    rule = build_rule

    expect {
      post run_inbox_settings_rule_path(rule), as: :turbo_stream
    }.to change { rule.runs.count }.by(1)
      .and have_enqueued_job(EmailRuleRunJob)

    run = rule.runs.last
    expect(run.status).to eq("queued")
    expect(run.started_by).to eq(user)
    expect(run.workspace).to eq(workspace)
    expect(response).to have_http_status(:ok)
  end

  # --- match_count ---

  it "GET match_count returns JSON count without persisting" do
    # No email accounts in test DB, so count is 0 (workspace scoped)
    get match_count_inbox_settings_rules_path(format: :json),
        params: { criteria: { from: "@stripe.com" } }

    expect(response).to have_http_status(:ok)
    body = JSON.parse(response.body)
    expect(body).to have_key("count")
    expect(body["count"]).to be_a(Integer)
  end

  it "GET match_count with no conditions returns 0 and does not raise" do
    get match_count_inbox_settings_rules_path(format: :json),
        params: { criteria: {} }

    expect(response).to have_http_status(:ok)
    body = JSON.parse(response.body)
    expect(body["count"]).to eq(0)
  end

  it "GET match_count never persists a rule record" do
    expect {
      get match_count_inbox_settings_rules_path(format: :json),
          params: { criteria: { from: "@stripe.com" } }
    }.not_to change(EmailRule, :count)
  end

  # --- undo ---

  it "POST undo calls UndoRun and re-renders the rule row" do
    rule = build_rule
    run  = EmailRuleRun.create!(
      email_rule: rule,
      workspace: workspace,
      started_by: user,
      status: :completed,
      matched_count: 5,
      processed_count: 5,
      undoable: true,
      tagged_email_ids: [],
      archived_email_ids: [],
      marked_read_email_ids: [],
      moved_email_ids: []
    )

    expect(EmailRules::UndoRun).to receive(:call).with(run)

    post inbox_settings_undo_rule_run_path(rule_id: rule.id, id: run.id),
         as: :turbo_stream

    expect(response).to have_http_status(:ok)
  end

  it "POST undo for a run belonging to another workspace raises 404" do
    other_ws   = Workspace.create!(name: "Alien", slug: "alien-#{SecureRandom.hex(4)}")
    other_rule = other_ws.email_rules.create!(
      name: "Alien rule",
      criteria: { "from" => [ "@alien.com" ] },
      archive: true, mark_read: false, enabled: true
    )
    other_run = EmailRuleRun.create!(
      email_rule: other_rule,
      workspace: other_ws,
      status: :completed,
      matched_count: 1,
      processed_count: 1,
      undoable: true,
      tagged_email_ids: [],
      archived_email_ids: [],
      marked_read_email_ids: [],
      moved_email_ids: []
    )

    # set_rule scopes to current workspace — accessing another workspace's rule 404s.
    post inbox_settings_undo_rule_run_path(rule_id: other_rule.id, id: other_run.id),
         as: :turbo_stream
    expect(response).to have_http_status(:not_found)
  end
end
