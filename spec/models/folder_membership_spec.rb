require "rails_helper"

RSpec.describe FolderMembership, type: :model do
  let(:workspace) { create(:workspace) }
  let(:folder) { create(:mail_folder, workspace: workspace) }
  let(:document) { create(:document, workspace: workspace) }

  it "places a document into a folder (both directions)" do
    folder.documents << document
    expect(folder.reload.documents).to include(document)
    expect(document.reload.mail_folders).to include(folder)
  end

  it "rejects the same document in the same folder twice" do
    folder.documents << document
    dup = FolderMembership.new(mail_folder: folder, folderable: document)
    expect(dup).not_to be_valid
  end

  it "allows the same document in different folders" do
    other = create(:mail_folder, workspace: workspace, name: "Other")
    folder.documents << document
    other.documents << document
    expect(document.reload.mail_folders).to contain_exactly(folder, other)
  end

  it "is removed when its folder is destroyed, leaving the document" do
    folder.documents << document
    expect { folder.destroy }.to change(FolderMembership, :count).by(-1)
    expect(Document.exists?(document.id)).to be(true)
  end

  it "is removed when its document is destroyed" do
    folder.documents << document
    expect { document.destroy }.to change(FolderMembership, :count).by(-1)
  end
end
