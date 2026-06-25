require "rails_helper"

RSpec.describe Campbooks::FolderPane, type: :component do
  def render_pane(**opts)
    ApplicationController.render(described_class.new(**opts), layout: false)
  end

  let(:system_folders) do
    [ { id: nil, name: "Inbox", count: 5 }, { id: "sent", name: "Sent", count: 0 }, { id: "drafts", name: "Drafts", count: 1 } ]
  end

  it "renders system folders, custom folders, and the new-folder action" do
    custom = [ build_stubbed(:mail_folder, name: "Receipts", icon: "currency-dollar") ]
    html = render_pane(system_folders: system_folders, custom_folders: custom, current_folder: nil)
    expect(html).to include("Inbox", "Receipts", "New folder")
    expect(html).to include('id="pane_custom_folders"')
  end

  it "excludes Sent/Drafts as drop targets but keeps the others" do
    html = render_pane(system_folders: system_folders, custom_folders: [], current_folder: nil)
    expect(html).not_to match(/folder-name="Sent"[^>]*mail-folder-drop-target/)
    expect(html).not_to match(/folder-name="Drafts"[^>]*mail-folder-drop-target/)
    expect(html).to match(/folder-name="Inbox"[^>]*mail-folder-drop-target/)
  end

  it "renders both an expanded panel and a collapsed rail" do
    html = render_pane(system_folders: system_folders, custom_folders: [], current_folder: nil)
    expect(html).to include('data-folder-pane-target="panel"')
    expect(html).to include('data-folder-pane-target="rail"')
  end
end
