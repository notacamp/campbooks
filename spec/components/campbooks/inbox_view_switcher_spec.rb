require "rails_helper"

# The inbox view switcher offers the Board (status kanban) segment only when the
# board feature is enabled (Features.email_board?); otherwise just Default + List.
# This pairs with the inbox-layout Stimulus controller, which derives its valid
# layouts from the rendered buttons, so dropping the segment here is sufficient.
RSpec.describe Campbooks::InboxViewSwitcher, type: :component do
  def render_switcher
    ApplicationController.render(described_class.new, layout: false)
  end

  it "offers Default, List and Board when the board feature is enabled" do
    allow(Features).to receive(:email_board?).and_return(true)

    html = render_switcher
    expect(html).to include('data-layout="default"')
    expect(html).to include('data-layout="list"')
    expect(html).to include('data-layout="board"')
  end

  it "drops the Board segment when the board feature is disabled" do
    allow(Features).to receive(:email_board?).and_return(false)

    html = render_switcher
    expect(html).to include('data-layout="default"')
    expect(html).to include('data-layout="list"')
    expect(html).not_to include('data-layout="board"')
  end
end
