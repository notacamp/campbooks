require "rails_helper"

RSpec.describe MfaEmailChallenge, type: :model do
  let(:user) { create(:user) }

  describe ".start_for!" do
    it "creates a 6-digit challenge and returns the plaintext code" do
      challenge, code = described_class.start_for!(user)

      expect(code).to match(/\A\d{6}\z/)
      expect(challenge).to be_persisted
      expect(challenge.attempts).to eq(0)
      expect(challenge.expires_at).to be_within(5.seconds).of(MfaEmailChallenge::TTL.from_now)
    end

    it "stores only the digest" do
      challenge, code = described_class.start_for!(user)

      expect(challenge.code_digest).not_to eq(code)
      expect(BCrypt::Password.new(challenge.code_digest)).to eq(code)
    end

    it "replaces the existing challenge instead of adding a second" do
      described_class.start_for!(user)

      expect { described_class.start_for!(user) }
        .not_to change { user.mfa_email_challenges.count }.from(1)
    end
  end

  describe "#verify" do
    it "returns true for the right code without burning an attempt" do
      challenge, code = described_class.start_for!(user)

      expect(challenge.verify(code)).to be(true)
      expect(challenge.reload.attempts).to eq(0)
    end

    it "returns false and increments attempts for a wrong code" do
      challenge, _code = described_class.start_for!(user)

      expect(challenge.verify("000000")).to be(false)
      expect(challenge.reload.attempts).to eq(1)
    end
  end

  describe "#expired? / #attempts_exhausted?" do
    it "is expired once past the TTL" do
      challenge, _ = described_class.start_for!(user)
      challenge.update!(expires_at: 1.second.ago)

      expect(challenge).to be_expired
    end

    it "is exhausted at MAX_ATTEMPTS" do
      challenge, _ = described_class.start_for!(user)
      challenge.update!(attempts: MfaEmailChallenge::MAX_ATTEMPTS)

      expect(challenge).to be_attempts_exhausted
    end
  end
end
