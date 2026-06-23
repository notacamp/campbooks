require "rails_helper"

RSpec.describe BetaCode, type: :model do
  describe "code generation" do
    it "auto-generates a formatted code on create" do
      expect(BetaCode.create!.code).to match(/\A[A-Z0-9]{4}-[A-Z0-9]{4}\z/)
    end

    it "generates distinct codes" do
      codes = Array.new(20) { BetaCode.create!.code }
      expect(codes.uniq.size).to eq(20)
    end
  end

  describe ".redeemable" do
    it "excludes redeemed and expired codes" do
      available = BetaCode.create!
      redeemed = BetaCode.create!
      redeemed.redeem!(create(:user))
      expired = BetaCode.create!(expires_at: 1.hour.ago)

      expect(BetaCode.redeemable).to include(available)
      expect(BetaCode.redeemable).not_to include(redeemed, expired)
    end
  end

  describe "#redeem!" do
    it "claims the code once and refuses a second time" do
      code = BetaCode.create!
      user = create(:user)

      expect(code.redeem!(user)).to be(true)
      expect(code.reload).to be_redeemed
      expect(code.redeemed_by).to eq(user)
      expect(code.redeem!(create(:user))).to be(false)
    end
  end

  describe ".find_redeemable" do
    it "matches case-insensitively, trims, and ignores hyphen/spacing, rejecting unknown codes" do
      code = BetaCode.create!
      bare = code.code.delete("-")

      expect(BetaCode.find_redeemable("  #{code.code.downcase}  ")).to eq(code)
      expect(BetaCode.find_redeemable(bare)).to eq(code)                       # dropped hyphen
      expect(BetaCode.find_redeemable(bare.downcase)).to eq(code)              # dropped hyphen + lowercase
      expect(BetaCode.find_redeemable("#{bare[0, 4]} #{bare[4, 4]}")).to eq(code) # space instead of hyphen
      expect(BetaCode.find_redeemable("NOPE-0000")).to be_nil
      expect(BetaCode.find_redeemable(nil)).to be_nil
    end
  end
end
