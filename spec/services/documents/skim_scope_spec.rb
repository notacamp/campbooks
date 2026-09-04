# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::SkimScope do
  let(:workspace) { create(:workspace) }
  let(:user_a)    { create(:user, workspace: workspace) }
  let(:user_b)    { create(:user, workspace: workspace) }

  def reattach(doc, filename:, content_type:)
    doc.original_file.attach(io: StringIO.new("x"), filename: filename, content_type: content_type)
    doc
  end

  it "includes reviewable documents (the factory default is a PDF)" do
    doc = create(:document, :in_review, workspace: workspace)
    expect(described_class.for(workspace, user_a).map(&:id)).to include(doc.id)
  end

  it "excludes non-document attachments -- calendar invites, archives, raw emails, DMARC reports" do
    ics   = reattach(create(:document, :in_review, workspace: workspace), filename: "invite.ics", content_type: "text/calendar")
    zip   = reattach(create(:document, :in_review, workspace: workspace), filename: "a.zip",      content_type: "application/zip")
    eml   = reattach(create(:document, :in_review, workspace: workspace), filename: "m.eml",      content_type: "message/rfc822")
    dmarc = reattach(create(:document, :in_review, workspace: workspace), filename: "r.xml",      content_type: "application/xhtml+xml")
    keep  = create(:document, :in_review, workspace: workspace) # PDF

    ids = described_class.for(workspace, user_a).map(&:id)
    expect(ids).to include(keep.id)
    expect(ids).not_to include(ics.id, zip.id, eml.id, dmarc.id)
  end

  it "orders most-uncertain first (lowest confidence, NULLs first)" do
    high = create(:document, :in_review, workspace: workspace, ai_confidence_score: 0.9)
    low  = create(:document, :in_review, workspace: workspace, ai_confidence_score: 0.1)
    none = create(:document, :in_review, workspace: workspace, ai_confidence_score: nil)

    expect(described_class.for(workspace, user_a).map(&:id)).to eq([ none.id, low.id, high.id ])
  end

  it "returns nothing for a nil workspace" do
    expect(described_class.for(nil, user_a)).to eq(Document.none)
  end

  it "returns nothing for a nil user (fail closed)" do
    create(:document, :in_review, workspace: workspace)
    expect(described_class.for(workspace, nil)).to eq(Document.none)
  end

  describe "folder access control" do
    it "includes an unfiled needs-review doc for both users (normal case not broken)" do
      unfiled = create(:document, :in_review, workspace: workspace)

      ids_a = described_class.for(workspace, user_a).map(&:id)
      ids_b = described_class.for(workspace, user_b).map(&:id)

      expect(ids_a).to include(unfiled.id)
      expect(ids_b).to include(unfiled.id)
    end

    it "hides a doc in a restricted folder from a user without read access (closes the leak)" do
      restricted = create(:mail_folder, workspace: workspace, restricted: true)
      restricted.mail_folder_users.create!(user: user_a, can_read: true)
      doc = create(:document, :in_review, workspace: workspace)
      restricted.folder_memberships.create!(folderable: doc)

      ids_a = described_class.for(workspace, user_a).map(&:id)
      ids_b = described_class.for(workspace, user_b).map(&:id)

      expect(ids_a).to include(doc.id)
      expect(ids_b).not_to include(doc.id)
    end

    it "includes a doc in an open folder for both users" do
      open_folder = create(:mail_folder, workspace: workspace)
      doc = create(:document, :in_review, workspace: workspace)
      open_folder.folder_memberships.create!(folderable: doc)

      ids_a = described_class.for(workspace, user_a).map(&:id)
      ids_b = described_class.for(workspace, user_b).map(&:id)

      expect(ids_a).to include(doc.id)
      expect(ids_b).to include(doc.id)
    end

    it "shows a restricted-folder doc to an admin" do
      admin = create(:user, workspace: workspace, role: :admin)
      restricted = create(:mail_folder, workspace: workspace, restricted: true)
      doc = create(:document, :in_review, workspace: workspace)
      restricted.folder_memberships.create!(folderable: doc)

      expect(described_class.for(workspace, admin).map(&:id)).to include(doc.id)
    end
  end
end
