require "rails_helper"

RSpec.describe Emails::WatchRenewalJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace, provider: :google) }
  let(:client)    { instance_double(Google::MailClient) }

  before do
    ActiveJob::Base.queue_adapter.enqueued_jobs.clear
    allow_any_instance_of(EmailAccount).to receive(:mail_client).and_return(client)
  end

  context "when Gmail push is not configured" do
    before { allow(Emails::GmailPush).to receive(:configured?).and_return(false) }

    it "is a complete no-op and never calls the mail client" do
      expect(client).not_to receive(:watch)
      described_class.perform_now
    end
  end

  context "when Gmail push is configured" do
    let(:expiry_time) { 7.days.from_now }

    before do
      allow(Emails::GmailPush).to receive(:configured?).and_return(true)
      allow(Emails::GmailPush).to receive(:topic).and_return("projects/proj/topics/gmail-push")
    end

    context "with an account that has no active watch (push_watch_expires_at nil)" do
      before { account.update_columns(push_watch_expires_at: nil) }

      it "calls watch and stores the returned expiry" do
        allow(client).to receive(:watch).with("projects/proj/topics/gmail-push")
          .and_return({ history_id: "100", expires_at: expiry_time })

        described_class.perform_now

        expect(account.reload.push_watch_expires_at).to be_within(1.second).of(expiry_time)
      end
    end

    context "with an account whose watch expires soon (within 24h)" do
      before { account.update_columns(push_watch_expires_at: 12.hours.from_now) }

      it "renews the watch and updates expires_at" do
        allow(client).to receive(:watch).and_return({ history_id: "101", expires_at: expiry_time })

        described_class.perform_now

        expect(account.reload.push_watch_expires_at).to be_within(1.second).of(expiry_time)
      end
    end

    context "with an account that already has a fresh watch (> 24h remaining)" do
      before { account.update_columns(push_watch_expires_at: 3.days.from_now) }

      it "skips the account and does not call watch" do
        expect(client).not_to receive(:watch)
        described_class.perform_now
      end
    end

    context "when one account raises AuthenticationError" do
      # Use let! for both accounts so find_each sees two rows. `let(:account)`
      # is lazy and would never be instantiated in a test that doesn't reference
      # it explicitly, leaving only other_account in the DB.
      let!(:failing_account) do
        create(:email_account, workspace: create(:workspace), provider: :google,
               push_watch_expires_at: nil)
      end
      let!(:succeeding_account) do
        create(:email_account, workspace: create(:workspace), provider: :google,
               push_watch_expires_at: nil)
      end

      before do
        # First call raises; subsequent calls return a normal result. Using a
        # call-count closure so the two separate stub definitions don't silently
        # override each other (in RSpec, a later allow overrides an earlier one).
        calls = 0
        allow(client).to receive(:watch) do
          calls += 1
          if calls == 1
            raise AuthenticationError, "token expired"
          else
            { history_id: "102", expires_at: expiry_time }
          end
        end
      end

      it "logs the error and continues to the next account" do
        expect(Rails.logger).to receive(:warn).with(/WatchRenewalJob.*auth error/)

        described_class.perform_now

        # One account raised, but the other must still be renewed.
        renewed = [ failing_account, succeeding_account ].count do |a|
          a.reload.push_watch_expires_at.present?
        end
        expect(renewed).to eq(1)
      end
    end

    context "when one account raises an unexpected error" do
      before { allow(client).to receive(:watch).and_raise(RuntimeError, "network blip") }

      it "logs the error and does not re-raise" do
        expect(Rails.logger).to receive(:error)
          .with(/WatchRenewalJob.*unexpected error.*#{account.id}/)
        expect { described_class.perform_now }.not_to raise_error
      end
    end
  end
end
