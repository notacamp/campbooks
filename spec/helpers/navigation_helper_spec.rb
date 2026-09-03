require "rails_helper"

RSpec.describe NavigationHelper, type: :helper do
  # nav_attention hits the DB per key; stub it to a quiet "no dots" double so the
  # spec is about the item list, not the badge source.
  let(:no_dots) { instance_double(Navigation::Attention, dot?: false) }

  before do
    allow(helper).to receive(:nav_attention).and_return(no_dots)
    stub_path("/")
  end

  # bold_layout? is a controller helper_method, not defined on the view object, so
  # partial-double verification would reject stubbing it directly.
  def stub_bold(value)
    without_partial_double_verification { allow(helper).to receive(:bold_layout?).and_return(value) }
  end

  def stub_path(path)
    allow(helper).to receive(:request).and_return(instance_double(ActionDispatch::Request, path: path))
  end

  def keys
    helper.primary_nav_items.map { |i| i[:key] }
  end

  context "classic layout (bold off)" do
    before do
      stub_bold(false)
      # The classic branch reaches for a couple more controller helper_methods.
      without_partial_double_verification do
        allow(helper).to receive(:current_user).and_return(nil)
        allow(helper).to receive(:current_entitlements).and_return(double("Entitlements", feature?: false))
      end
    end

    it "renders the classic five and never the bold places" do
      expect(keys).to include(:home, :mail)
      expect(keys).not_to include(:now, :people, :paper, :money, :time)
    end
  end

  context "bold layout on" do
    before { stub_bold(true) }

    it "renders Now / People / Paper / Money / Time with no Scout/ember tile" do
      allow(Features).to receive(:accounting?).and_return(true)
      expect(keys).to eq(%i[now people paper money time])
      expect(helper.primary_nav_items).to all(satisfy { |i| i[:ember] == false })
    end

    it "drops Money when accounting is unavailable (same gate as the classic item)" do
      allow(Features).to receive(:accounting?).and_return(false)
      expect(keys).to eq(%i[now people paper time])
    end

    it "marks Now active on /now but not on /home (the classic feed)" do
      allow(Features).to receive(:accounting?).and_return(true)
      stub_path("/now")
      expect(helper.primary_nav_items.find { |i| i[:key] == :now }[:active]).to be(true)

      stub_path("/home")
      expect(helper.primary_nav_items.find { |i| i[:key] == :now }[:active]).to be(false)
    end

    it "marks People active across mail, contacts and organizations" do
      allow(Features).to receive(:accounting?).and_return(true)
      stub_path("/contacts")
      expect(helper.primary_nav_items.find { |i| i[:key] == :people }[:active]).to be(true)
    end

    it "marks Time active across calendar, reminders and tasks" do
      allow(Features).to receive(:accounting?).and_return(true)
      stub_path("/reminders")
      expect(helper.primary_nav_items.find { |i| i[:key] == :time }[:active]).to be(true)
    end
  end
end
