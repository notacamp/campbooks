require "rails_helper"

# Unit tests for the ProviderSyncDeactivation concern, exercised through
# EmailAccount (primary host) with one cross-check on CalendarAccount.
# Converted from test/models/provider_sync_deactivation_test.rb.
RSpec.describe "ProviderSyncDeactivation" do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  describe "#deactivate_for!" do
    it "marks the account inactive and stores the reason" do
      expect(account.active?).to be(true)

      account.deactivate_for!(:mail_service_unavailable)
      account.reload

      expect(account.active?).to be(false)
      expect(account.deactivation_reason).to eq("mail_service_unavailable")
    end

    it "accepts a string reason" do
      account.deactivate_for!("mail_service_unavailable")
      account.reload

      expect(account.active?).to be(false)
      expect(account.deactivation_reason).to eq("mail_service_unavailable")
    end

    it "is a no-op when account is already inactive (does not overwrite the existing reason)" do
      account.deactivate_for!(:mail_service_unavailable)
      expect(account.reload.deactivation_reason).to eq("mail_service_unavailable")

      expect { account.deactivate_for!(:calendar_service_unavailable) }
        .not_to change { EmailAccount.where(active: false).count }

      expect(account.reload.deactivation_reason).to eq("mail_service_unavailable")
    end
  end

  describe "#deactivated_for_service?" do
    it "is true when inactive with a reason" do
      account.deactivate_for!(:mail_service_unavailable)
      account.reload
      expect(account.deactivated_for_service?).to be(true)
    end

    it "is false when the account is active" do
      expect(account.active?).to be(true)
      expect(account.deactivated_for_service?).to be(false)
    end

    it "is false when inactive but deactivation_reason is nil" do
      # Plain disconnect (token revoked) does not go through deactivate_for!.
      account.update!(active: false)
      expect(account.deactivation_reason).to be_nil
      expect(account.deactivated_for_service?).to be(false)
    end
  end

  describe "before_save hook: reactivation clears the deactivation reason" do
    it "clears the deactivation reason when the account is reactivated" do
      account.deactivate_for!(:mail_service_unavailable)
      expect(account.reload.deactivation_reason).to eq("mail_service_unavailable")

      account.update!(active: true)
      account.reload

      expect(account.active?).to be(true)
      expect(account.deactivation_reason).to be_nil
    end
  end

  describe "#deactivation_reason_label" do
    it "returns the English translation for mail_service_unavailable" do
      account.deactivate_for!(:mail_service_unavailable)
      label = account.deactivation_reason_label
      expect(label).not_to be_nil
      expect(label.downcase).to include("gmail")
    end

    it "returns nil when deactivation_reason is blank" do
      expect(account.deactivation_reason).to be_nil
      expect(account.deactivation_reason_label).to be_nil
    end

    it "returns nil for an unrecognized reason (no translation key)" do
      account.update_columns(active: false, deactivation_reason: "unknown_future_reason")
      expect(account.deactivation_reason_label).to be_nil
    end
  end

  describe "CalendarAccount includes the concern too" do
    let(:cal_account) { create(:calendar_account, workspace: workspace) }

    it "can be deactivated with calendar_service_unavailable" do
      cal_account.deactivate_for!(:calendar_service_unavailable)
      cal_account.reload

      expect(cal_account.active?).to be(false)
      expect(cal_account.deactivated_for_service?).to be(true)
      expect(cal_account.deactivation_reason).to eq("calendar_service_unavailable")

      label = cal_account.deactivation_reason_label
      expect(label).not_to be_nil
      expect(label.downcase).to include("calendar")
    end

    it "clears the reason on reactivation" do
      cal_account.deactivate_for!(:calendar_service_unavailable)
      cal_account.update!(active: true)
      expect(cal_account.reload.deactivation_reason).to be_nil
    end
  end
end
