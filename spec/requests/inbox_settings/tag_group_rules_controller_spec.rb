# frozen_string_literal: true

require "rails_helper"

# Controller-level integration tests for InboxSettings::TagGroupsController rule
# CRUD and for rules-only group support. Verifies that rules are created,
# updated, destroyed, and that the bulk-action validate_group! gate accepts
# groups that exist only through rules.
RSpec.describe "InboxSettings::TagGroups (rules)", type: :request do
  before do
    @workspace = Workspace.create!(name: "RuleCtrl WS #{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
    Tags::DefaultGroups.provision!(@workspace)
    sign_in_as(@user)
  end

  # ---- create: rules-only group (no tag_ids) --------------------------------

  it "create saves a rules-only group with a sender rule" do
    post inbox_settings_tag_groups_path, params: {
      name:  "Vendors",
      rules: [ { rule_type: "sender", value: "@vendor.example" } ]
    }

    expect(response).to redirect_to(inbox_settings_tag_groups_path)
    rule = @workspace.inbox_group_rules.find_by(group_name: "Vendors")
    expect(rule).not_to be_nil
    expect(rule.rule_type).to eq("sender")
    expect(rule.value).to eq("@vendor.example")
  end

  it "create with both tags and rules saves both" do
    tag = @workspace.tags.create!(name: "Finance #{SecureRandom.hex(2)}", color: "#aabb00")

    post inbox_settings_tag_groups_path, params: {
      name:    "Finance",
      tag_ids: [ tag.id ],
      rules:   [ { rule_type: "sender", value: "invoices@acme.example" } ]
    }

    expect(response).to redirect_to(inbox_settings_tag_groups_path)
    expect(tag.reload.group_name).to eq("Finance")
    rule = @workspace.inbox_group_rules.find_by(group_name: "Finance", rule_type: "sender")
    expect(rule).not_to be_nil
  end

  it "create with a blank name is a no-op (does not create rules)" do
    post inbox_settings_tag_groups_path, params: {
      name:  "",
      rules: [ { rule_type: "sender", value: "@vendor.example" } ]
    }
    expect(@workspace.inbox_group_rules.count).to eq(0)
  end

  # ---- update ---------------------------------------------------------------

  it "update replaces existing rules for the group" do
    @workspace.inbox_group_rules.create!(
      group_name: "Vendors", rule_type: "sender", value: "@old.example"
    )

    patch inbox_settings_tag_groups_path, params: {
      original_name: "Vendors",
      name:          "Vendors",
      rules:         [ { rule_type: "sender", value: "@new.example" } ]
    }

    expect(response).to redirect_to(inbox_settings_tag_groups_path)
    rules = @workspace.inbox_group_rules.where(group_name: "Vendors")
    expect(rules.count).to eq(1)
    expect(rules.first.value).to eq("@new.example")
  end

  it "update renames the group on both tags and rules" do
    tag = @workspace.tags.create!(name: "Finance #{SecureRandom.hex(2)}", color: "#aabb00",
                                  group_name: "OldName")
    @workspace.inbox_group_rules.create!(
      group_name: "OldName", rule_type: "sender", value: "@vendor.example"
    )

    patch inbox_settings_tag_groups_path, params: {
      original_name: "OldName",
      name:          "NewName",
      tag_ids:       [ tag.id ],
      rules:         [ { rule_type: "sender", value: "@vendor.example" } ]
    }

    expect(response).to redirect_to(inbox_settings_tag_groups_path)
    expect(tag.reload.group_name).to eq("NewName")
    expect(@workspace.inbox_group_rules.where(group_name: "OldName").count).to eq(0)
    expect(@workspace.inbox_group_rules.where(group_name: "NewName").count).to eq(1)
  end

  # ---- destroy --------------------------------------------------------------

  it "destroy removes both tags and rules for the group" do
    tag = @workspace.tags.create!(name: "Finance #{SecureRandom.hex(2)}", color: "#aabb00",
                                  group_name: "Vendors")
    @workspace.inbox_group_rules.create!(
      group_name: "Vendors", rule_type: "sender", value: "@vendor.example"
    )

    delete inbox_settings_tag_groups_path, params: { group: "Vendors" }

    expect(response).to redirect_to(inbox_settings_tag_groups_path)
    expect(tag.reload.group_name).to be_nil
    expect(@workspace.inbox_group_rules.where(group_name: "Vendors").count).to eq(0)
  end

  # ---- bulk actions on rules-only groups ------------------------------------

  it "archive_all accepts a rules-only group (validate_group! passes)" do
    @workspace.inbox_group_rules.create!(
      group_name: "RulesOnly", rule_type: "sender", value: "@vendor.example"
    )

    post tag_group_archive_all_path, params: { group: "RulesOnly" }

    expect(response).not_to have_http_status(:bad_request)
  end

  it "mark_all_read accepts a rules-only group (validate_group! passes)" do
    @workspace.inbox_group_rules.create!(
      group_name: "RulesOnly", rule_type: "sender", value: "@vendor.example"
    )

    post tag_group_mark_all_read_path, params: { group: "RulesOnly" }

    expect(response).not_to have_http_status(:bad_request)
  end

  # ---- multiple rule types in one group ------------------------------------

  it "create saves multiple rules of different types" do
    dt = @workspace.document_types.create!(name: "Invoice #{SecureRandom.hex(2)}", color: "#aabbcc")

    post inbox_settings_tag_groups_path, params: {
      name:  "Multi",
      rules: [
        { rule_type: "sender", value: "billing@acme.example" },
        { rule_type: "document_type", value: dt.id.to_s }
      ]
    }

    expect(response).to redirect_to(inbox_settings_tag_groups_path)
    rules = @workspace.inbox_group_rules.where(group_name: "Multi").order(:rule_type)
    expect(rules.count).to eq(2)
    expect(rules.map(&:rule_type)).to contain_exactly("document_type", "sender")
  end

  # ---- skips invalid rule_types -------------------------------------------

  it "create silently skips rules with an invalid rule_type" do
    post inbox_settings_tag_groups_path, params: {
      name:  "Test",
      rules: [ { rule_type: "unknown_type", value: "foo" } ]
    }

    expect(@workspace.inbox_group_rules.count).to eq(0)
  end

  # ---- regression: the REAL browser form submits INDEXED inputs --------------
  #
  # The form renders rules[0][rule_type], rules[1][...] — which Rack parses into
  # an index-keyed hash ({ "0" => {...} }), NOT an array. The array-shaped specs
  # above all passed while the live form silently dropped every rule. These drive
  # the exact shape a real submission produces.

  it "create saves rules submitted in the real indexed-hash shape" do
    post inbox_settings_tag_groups_path, params: {
      name:  "Vendors",
      rules: { "0" => { rule_type: "sender", value: "@vendor.example" },
               "1" => { rule_type: "query",  value: "is:unread" } }
    }

    expect(response).to redirect_to(inbox_settings_tag_groups_path)
    rules = @workspace.inbox_group_rules.where(group_name: "Vendors")
    expect(rules.count).to eq(2)
    expect(rules.map(&:rule_type)).to contain_exactly("query", "sender")
  end

  it "update replaces rules submitted in the real indexed-hash shape" do
    @workspace.inbox_group_rules.create!(
      group_name: "Vendors", rule_type: "sender", value: "@old.example"
    )

    patch inbox_settings_tag_groups_path, params: {
      original_name: "Vendors",
      name:          "Vendors",
      rules:         { "0" => { rule_type: "sender", value: "@new.example" } }
    }

    rules = @workspace.inbox_group_rules.where(group_name: "Vendors")
    expect(rules.count).to eq(1)
    expect(rules.first.value).to eq("@new.example")
  end

  # ---- regression: a stray blank-named group must not render a dead row -------

  it "index does not render an un-editable blank-named group row" do
    tag = @workspace.tags.create!(name: "Orphan #{SecureRandom.hex(2)}", color: "#123456")
    tag.update_column(:group_name, "") # legacy orphan: grouped but blank-named

    get inbox_settings_tag_groups_path

    expect(response).to have_http_status(:ok)
    # A blank name would produce an edit link ending in "?group=" — never render it.
    expect(response.body).not_to include('tag_groups/edit?group="')
  end

  it "edit of a blank/unknown group redirects instead of rendering a dead frame" do
    get inbox_settings_edit_tag_group_path(group: "")
    expect(response).to redirect_to(inbox_settings_tag_groups_path)
  end
end
