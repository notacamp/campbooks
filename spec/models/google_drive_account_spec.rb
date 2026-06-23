require "rails_helper"

RSpec.describe GoogleDriveAccount, type: :model do
  describe "#full_access?" do
    it "is true when the full drive scope was granted" do
      account = GoogleDriveAccount.new(scopes: "https://www.googleapis.com/auth/drive https://www.googleapis.com/auth/userinfo.email")
      expect(account.full_access?).to be(true)
    end

    it "is false for the legacy drive.file scope only" do
      account = GoogleDriveAccount.new(scopes: "https://www.googleapis.com/auth/drive.file")
      expect(account.full_access?).to be(false)
    end

    it "is false when scopes are unknown (pre-migration rows)" do
      expect(GoogleDriveAccount.new(scopes: nil).full_access?).to be(false)
    end
  end
end
