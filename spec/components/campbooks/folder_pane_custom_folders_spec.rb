require "rails_helper"

RSpec.describe Campbooks::FolderPaneCustomFolders, type: :component do
  def render_section(folders, current: nil)
    ApplicationController.render(described_class.new(custom_folders: folders, current_folder: current), layout: false)
  end

  it "always renders the live-refresh container" do
    expect(render_section([])).to include('id="pane_custom_folders"')
  end

  it "renders an edit dialog per folder with the current icon preselected and patch + delete forms" do
    folder = build_stubbed(:mail_folder, name: "Receipts", icon: "currency-dollar")
    html = render_section([ folder ])
    expect(html).to include("Receipts")
    expect(html).to include("<dialog")
    expect(html).to include('value="patch"')
    expect(html).to include('value="delete"')
    expect(html).to match(/value="currency-dollar"[^>]*checked/)
    expect(html).to include("folder-edit#open")
  end
end
