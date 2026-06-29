require "rails_helper"

RSpec.describe "Folder permissions (Files Phase 3)", type: :model do
  let(:workspace) { create(:workspace) }
  let(:member)    { create(:user, workspace: workspace) }            # plain member
  let(:other)     { create(:user, workspace: workspace) }            # plain member, no access
  let(:admin)     { create(:user, workspace: workspace, role: :admin) }

  describe "MailFolder.accessible_to" do
    it "shows open folders to everyone but hides restricted folders from non-members" do
      open_folder = create(:mail_folder, workspace: workspace)
      restricted  = create(:mail_folder, workspace: workspace, restricted: true)
      restricted.mail_folder_users.create!(user: member, owner: true, can_read: true, can_manage: true)

      expect(workspace.mail_folders.accessible_to(member)).to include(open_folder, restricted)
      expect(workspace.mail_folders.accessible_to(other)).to include(open_folder)
      expect(workspace.mail_folders.accessible_to(other)).not_to include(restricted)
      expect(workspace.mail_folders.accessible_to(admin)).to include(restricted)
    end
  end

  describe "MailFolder#readable_by? / #manageable_by?" do
    it "gates on membership + admin" do
      restricted = create(:mail_folder, workspace: workspace, restricted: true)
      restricted.mail_folder_users.create!(user: member, can_read: true, can_manage: true)

      expect(restricted.readable_by?(member)).to be(true)
      expect(restricted.readable_by?(other)).to be(false)
      expect(restricted.readable_by?(admin)).to be(true)
      expect(restricted.manageable_by?(member)).to be(true)
      expect(restricted.manageable_by?(other)).to be(false)
    end
  end

  describe "Document.accessible_to inheritance" do
    it "hides a document filed only in a restricted folder from non-members" do
      restricted = create(:mail_folder, workspace: workspace, restricted: true)
      restricted.mail_folder_users.create!(user: member, can_read: true)
      doc = create(:document, :other, workspace: workspace)
      restricted.folder_memberships.create!(folderable: doc)

      expect(Document.accessible_to(member)).to include(doc)
      expect(Document.accessible_to(other)).not_to include(doc)
      expect(Document.accessible_to(admin)).to include(doc)
    end

    it "keeps unfiled documents and documents in open folders visible to everyone" do
      unfiled = create(:document, :other, workspace: workspace)
      open_folder = create(:mail_folder, workspace: workspace)
      filed_open = create(:document, :other, workspace: workspace)
      open_folder.folder_memberships.create!(folderable: filed_open)

      expect(Document.accessible_to(other)).to include(unfiled, filed_open)
    end

    it "still shows a document that's also filed in an open folder" do
      restricted = create(:mail_folder, workspace: workspace, restricted: true)
      open_folder = create(:mail_folder, workspace: workspace)
      doc = create(:document, :other, workspace: workspace)
      restricted.folder_memberships.create!(folderable: doc)
      open_folder.folder_memberships.create!(folderable: doc)

      expect(Document.accessible_to(other)).to include(doc)
    end
  end
end
