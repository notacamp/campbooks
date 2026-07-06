# frozen_string_literal: true

require "rails_helper"

# The Groups panel: a "group" is the set of tags sharing a group_name, so the
# controller's create/rename/dissolve just rewrite that column across the
# workspace's tags. Names travel as params (never path segments).
RSpec.describe "InboxSettings::TagGroupsController", type: :request do
  before { sign_in_as(user) }

  let(:workspace) { Workspace.create!(name: "Groups Ctrl WS", slug: "groups-ctrl-#{SecureRandom.hex(4)}") }
  let(:user) do
    workspace.users.create!(
      name: "Grouper",
      email_address: "grouper-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  def make_tag(name, group: nil)
    workspace.tags.create!(name: name, color: "#0584da", group_name: group, source: :local, kind: :user, hidden: false)
  end

  describe "GET index" do
    it "lists the workspace's groups with their member tags" do
      make_tag("deals", group: "Promos")
      make_tag("news",  group: "Promos")
      make_tag("plain")

      get inbox_settings_tag_groups_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Promos")
      expect(response.body).to include("deals")
    end
  end

  describe "POST create" do
    it "puts the selected tags into the named group" do
      a = make_tag("deals")
      b = make_tag("news")
      c = make_tag("plain")

      post inbox_settings_tag_groups_path, params: { name: "Promos", tag_ids: [ a.id, b.id ] }

      expect(response).to redirect_to(inbox_settings_tag_groups_path)
      expect([ a.reload.group_name, b.reload.group_name ]).to eq([ "Promos", "Promos" ])
      expect(c.reload.group_name).to be_nil
    end

    it "moves a tag out of its previous group (a tag has one group)" do
      a = make_tag("deals", group: "Old")

      post inbox_settings_tag_groups_path, params: { name: "New", tag_ids: [ a.id ] }

      expect(a.reload.group_name).to eq("New")
    end

    it "is a no-op for a blank name or an empty selection" do
      a = make_tag("deals")

      post inbox_settings_tag_groups_path, params: { name: "  ", tag_ids: [ a.id ] }
      expect(a.reload.group_name).to be_nil

      post inbox_settings_tag_groups_path, params: { name: "Promos", tag_ids: [] }
      expect(a.reload.group_name).to be_nil
    end

    it "never touches another workspace's tags" do
      other_ws  = Workspace.create!(name: "Other WS", slug: "other-#{SecureRandom.hex(4)}")
      other_tag = other_ws.tags.create!(name: "deals", color: "#0584da", source: :local, kind: :user, hidden: false)

      post inbox_settings_tag_groups_path, params: { name: "Promos", tag_ids: [ other_tag.id ] }

      expect(other_tag.reload.group_name).to be_nil
    end
  end

  describe "PATCH update" do
    it "renames the group and replaces its membership" do
      a = make_tag("deals", group: "Promos")
      b = make_tag("news",  group: "Promos")
      c = make_tag("offers")

      patch inbox_settings_tag_groups_path,
            params: { original_name: "Promos", name: "Newsletters & promos", tag_ids: [ a.id, c.id ] }

      expect(response).to redirect_to(inbox_settings_tag_groups_path)
      expect(a.reload.group_name).to eq("Newsletters & promos")
      expect(c.reload.group_name).to eq("Newsletters & promos")
      # Deselected member is ungrouped.
      expect(b.reload.group_name).to be_nil
    end
  end

  describe "DELETE destroy" do
    it "dissolves the group but keeps the tags" do
      a = make_tag("deals", group: "Promos")
      b = make_tag("news",  group: "Promos")

      expect {
        delete inbox_settings_tag_groups_path, params: { group: "Promos" }
      }.not_to change(Tag, :count)

      expect(response).to redirect_to(inbox_settings_tag_groups_path)
      expect([ a.reload.group_name, b.reload.group_name ]).to eq([ nil, nil ])
    end
  end

  describe "GET edit" do
    it "renders the form for an existing group, preselecting its members" do
      make_tag("deals", group: "Promos")

      get inbox_settings_edit_tag_group_path(group: "Promos")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Promos")
      expect(response.body).to include("checked")
    end

    it "redirects back for an unknown group name" do
      get inbox_settings_edit_tag_group_path(group: "Nope")

      expect(response).to redirect_to(inbox_settings_tag_groups_path)
    end
  end
end
