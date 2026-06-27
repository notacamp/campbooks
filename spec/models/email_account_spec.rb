require "rails_helper"

RSpec.describe EmailAccount, type: :model do
  describe "associations" do
    it { is_expected.to have_many(:email_messages).dependent(:restrict_with_error) }
    it { is_expected.to have_many(:email_scan_logs).dependent(:restrict_with_error) }
  end

  describe "validations" do
    subject { build(:email_account) }

    it { is_expected.to validate_presence_of(:email_address) }
    it { is_expected.to validate_uniqueness_of(:email_address) }
    it { is_expected.to validate_presence_of(:refresh_token) }
    it { is_expected.to validate_length_of(:name).is_at_most(100) }

    it "allows a blank name" do
      expect(build(:email_account, name: nil)).to be_valid
    end
  end

  describe "scopes" do
    describe ".active" do
      it "returns only active accounts" do
        active = create(:email_account, active: true)
        inactive = create(:email_account, active: false, email_address: "other@test.com")

        expect(EmailAccount.active).to include(active)
        expect(EmailAccount.active).not_to include(inactive)
      end
    end

    describe ".actively_scanning" do
      it "includes accounts with a fresh scan claim" do
        account = create(:email_account, scanning: true, scan_started_at: 1.minute.ago)
        expect(EmailAccount.actively_scanning).to include(account)
      end

      it "excludes accounts whose scan claim has gone stale" do
        account = create(:email_account, scanning: true, scan_started_at: 20.minutes.ago)
        expect(EmailAccount.actively_scanning).not_to include(account)
      end

      it "excludes idle accounts" do
        account = create(:email_account, scanning: false)
        expect(EmailAccount.actively_scanning).not_to include(account)
      end
    end

    describe ".ordered" do
      it "sorts by display label (name, falling back to email) case-insensitively" do
        zed   = create(:email_account, name: "Zed",  email_address: "z@test.com")
        amy   = create(:email_account, name: "amy",  email_address: "a@test.com")
        blank = create(:email_account, name: nil,    email_address: "beth@test.com")

        # "amy" < "beth" (email fallback) < "Zed", ignoring case.
        expect(EmailAccount.ordered.to_a).to eq([ amy, blank, zed ])
      end

      it "is stable regardless of insertion/update order" do
        b = create(:email_account, name: "Bravo", email_address: "b@test.com")
        a = create(:email_account, name: "Alpha", email_address: "a@test.com")
        b.update!(last_scanned_at: Time.current) # a sync-style touch must not reshuffle

        expect(EmailAccount.ordered.to_a).to eq([ a, b ])
      end
    end
  end

  describe "#deactivate!" do
    it "sets active to false" do
      account = create(:email_account)
      account.deactivate!
      expect(account.reload.active).to be false
    end
  end

  describe "#record_scan!" do
    it "updates last_scanned_at" do
      account = create(:email_account)
      expect { account.record_scan! }.to change { account.reload.last_scanned_at }
    end
  end

  describe "display helpers" do
    describe "#display_name" do
      it "returns the email address when no name is set" do
        account = build(:email_account, name: nil, email_address: "team@acme.com")
        expect(account.display_name).to eq("team@acme.com")
      end

      it "returns the custom name when one is set" do
        account = build(:email_account, name: "Marketing", email_address: "team@acme.com")
        expect(account.display_name).to eq("Marketing")
      end

      it "falls back to the email when the name is blank" do
        account = build(:email_account, name: "   ", email_address: "team@acme.com")
        expect(account.display_name).to eq("team@acme.com")
      end
    end

    describe "#select_label" do
      it "returns just the email when no name is set" do
        account = build(:email_account, name: nil, email_address: "team@acme.com")
        expect(account.select_label).to eq("team@acme.com")
      end

      it "combines the name and email when a name is set" do
        account = build(:email_account, name: "Marketing", email_address: "team@acme.com")
        expect(account.select_label).to eq("Marketing (team@acme.com)")
      end
    end

    describe "#avatar_initial" do
      it "uses the first letter of the email when unnamed" do
        account = build(:email_account, name: nil, email_address: "team@acme.com")
        expect(account.avatar_initial).to eq("T")
      end

      it "uses the first letter of the name when named" do
        account = build(:email_account, name: "marketing", email_address: "team@acme.com")
        expect(account.avatar_initial).to eq("M")
      end
    end
  end

  describe "encryption" do
    it "encrypts refresh_token" do
      account = create(:email_account, refresh_token: "secret-token-123")
      account.reload
      expect(account.refresh_token).to eq("secret-token-123")
      # The DB column should not contain the plain text
      raw = ActiveRecord::Base.connection.execute(
        "SELECT refresh_token FROM email_accounts WHERE id = '#{account.id}'"
      ).first["refresh_token"]
      expect(raw).not_to include("secret-token-123")
    end
  end
end
