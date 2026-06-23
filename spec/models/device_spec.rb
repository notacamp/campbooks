require "rails_helper"

RSpec.describe Device, type: :model do
  let(:user) { create(:user) }

  it "maps the platform enum" do
    expect(Device.platforms).to eq("ios" => 0, "android" => 1)
  end

  it "requires a token" do
    device = build(:device, token: nil)
    expect(device).not_to be_valid
    expect(device.errors[:token]).to be_present
  end

  it "enforces token uniqueness" do
    create(:device, token: "dup")
    expect { create(:device, token: "dup") }.to raise_error(ActiveRecord::RecordInvalid)
  end

  describe ".register!" do
    it "creates a device for a new token and stamps activity" do
      device = Device.register!(user: user, platform: :ios, token: "abc", app_version: "1.0")

      expect(device).to be_persisted
      expect(device.user).to eq(user)
      expect(device).to be_ios
      expect(device.app_version).to eq("1.0")
      expect(device.last_active_at).to be_present
    end

    it "refreshes an existing token instead of duplicating" do
      Device.register!(user: user, platform: :ios, token: "abc")

      expect {
        Device.register!(user: user, platform: :ios, token: "abc", app_version: "2.0")
      }.not_to change(Device, :count)

      expect(Device.find_by(token: "abc").app_version).to eq("2.0")
    end

    it "moves a token to the new user when it re-registers under them" do
      other = create(:user)
      Device.register!(user: other, platform: :android, token: "shared")
      Device.register!(user: user, platform: :android, token: "shared")

      expect(Device.where(token: "shared").count).to eq(1)
      expect(Device.find_by(token: "shared").user).to eq(user)
    end

    it "raises on an unknown platform" do
      expect {
        Device.register!(user: user, platform: :windows, token: "x")
      }.to raise_error(ArgumentError)
    end
  end
end
