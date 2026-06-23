require "rails_helper"

RSpec.describe RecoveryCode, type: :model do
  let(:user) { create(:user) }

  describe ".regenerate_for!" do
    it "creates COUNT codes and returns the grouped plaintext once" do
      codes = described_class.regenerate_for!(user)

      expect(codes.size).to eq(RecoveryCode::COUNT)
      expect(codes).to all(match(/\A[a-z0-9]{5}-[a-z0-9]{5}\z/))
      expect(user.recovery_codes.count).to eq(RecoveryCode::COUNT)
    end

    it "persists only digests, never the plaintext" do
      codes = described_class.regenerate_for!(user)
      raw = codes.first.delete("-")

      expect(user.recovery_codes.pluck(:code_digest)).not_to include(raw, codes.first)
    end

    it "replaces any existing codes (old ones stop working)" do
      old = described_class.regenerate_for!(user)
      fresh = described_class.regenerate_for!(user)

      expect(user.recovery_codes.count).to eq(RecoveryCode::COUNT)
      expect(described_class.consume!(user, old.first)).to be_nil
      expect(described_class.consume!(user, fresh.first)).to be_present
    end
  end

  describe ".consume!" do
    let!(:codes) { described_class.regenerate_for!(user) }

    it "accepts a valid code and marks it single-use" do
      consumed = described_class.consume!(user, codes.first)

      expect(consumed.used_at).to be_present
      expect(described_class.consume!(user, codes.first)).to be_nil
      expect(user.recovery_codes.unused.count).to eq(RecoveryCode::COUNT - 1)
    end

    it "matches leniently — case-insensitive, hyphens/spaces ignored" do
      raw = codes.first.delete("-")

      expect(described_class.consume!(user, raw.upcase)).to be_present
    end

    it "returns nil for an unknown or blank code" do
      expect(described_class.consume!(user, "zzzzz-zzzzz")).to be_nil
      expect(described_class.consume!(user, "")).to be_nil
      expect(described_class.consume!(user, nil)).to be_nil
    end
  end
end
