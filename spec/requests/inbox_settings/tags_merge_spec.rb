# frozen_string_literal: true

require "rails_helper"

RSpec.describe "InboxSettings::TagsController#merge", type: :request do
  before { sign_in_as(user) }

  let(:workspace) { create(:workspace) }
  let(:user) { workspace.users.create!(name: "Merger", email_address: "merger-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let!(:source) { create(:tag, workspace: workspace, name: "Old Tag") }
  let!(:target) { create(:tag, workspace: workspace, name: "New Tag") }

  describe "GET /inbox_settings/tags/:id/merge" do
    it "renders the merge form" do
      get merge_inbox_settings_tag_path(source)
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(source.name)
    end
  end

  describe "POST /inbox_settings/tags/:id/commit_merge" do
    let(:account) { create(:email_account, workspace: workspace) }

    it "merges the source into the target and destroys the source" do
      post commit_merge_inbox_settings_tag_path(source),
           params: { into_tag_id: target.id },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(Tag.exists?(source.id)).to be false
    end

    it "renders an error when merging into itself" do
      post commit_merge_inbox_settings_tag_path(source),
           params: { into_tag_id: source.id },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(Tag.exists?(source.id)).to be true
    end

    it "cannot merge tags from another workspace" do
      other_ws     = create(:workspace)
      other_target = create(:tag, workspace: other_ws)

      post commit_merge_inbox_settings_tag_path(source),
           params: { into_tag_id: other_target.id },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(Tag.exists?(source.id)).to be true
    end
  end
end
