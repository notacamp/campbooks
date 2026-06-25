require "rails_helper"

RSpec.describe Campbooks::IconPicker, type: :component do
  def render_picker(**opts)
    ApplicationController.render(described_class.new(name: "mail_folder[icon]", **opts), layout: false)
  end

  it "renders a radio per icon plus the leading default option" do
    html = render_picker
    expect(html.scan('type="radio"').size).to eq(Campbooks::Icon::NAMES.size + 1)
    expect(html).to include('name="mail_folder[icon]"')
  end

  it "checks the default (blank) option when nothing is selected" do
    expect(render_picker).to match(/value=""[^>]*checked/)
  end

  it "checks the selected icon" do
    expect(render_picker(selected: "star")).to match(/value="star"[^>]*checked/)
  end
end
