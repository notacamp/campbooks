require "rails_helper"

RSpec.describe Emails::MessageUpserter do
  let(:account) { create(:email_account, provider: :zoho) }
  let(:scan_log) { create(:email_scan_log, email_account: account) }
  subject(:upserter) { described_class.new(account, scan_log: scan_log) }

  def msg(overrides = {})
    {
      "messageId" => "m1",
      "folderId" => "inbox",
      "fromAddress" => "sender@test.com",
      "toAddress" => "me@test.com",
      "subject" => "Hello",
      "summary" => "Preview",
      "hasAttachment" => "0",
      "receivedTime" => (Time.utc(2026, 1, 2, 3, 4, 5).to_i * 1000).to_s,
      "status" => "0",
      "flagid" => nil
    }.merge(overrides)
  end

  describe "creating a new message" do
    it "creates the EmailMessage, enqueues processing, and returns :created" do
      outcome = nil
      expect { outcome = upserter.upsert(msg) }
        .to change(EmailMessage, :count).by(1)
        .and have_enqueued_job(EmailProcessJob)
      expect(outcome).to eq(:created)

      m = EmailMessage.last
      expect(m.provider_message_id).to eq("m1")
      expect(m.provider_folder_id).to eq("inbox")
      expect(m.subject).to eq("Hello")
      expect(m.read).to be false
      expect(m.email_scan_log).to eq(scan_log)
      expect(m.received_at).to eq(Time.utc(2026, 1, 2, 3, 4, 5))
    end

    it "marks the message read when the provider reports it read (status '1')" do
      upserter.upsert(msg("status" => "1"))
      expect(EmailMessage.last.read).to be true
    end

    it "strips PG-incompatible NUL bytes from text fields" do
      # 0.chr is a literal NUL byte, built at runtime so there's no invisible NUL
      # in this source file.
      upserter.upsert(msg("subject" => "a#{0.chr}b"))
      expect(EmailMessage.last.subject).to eq("ab")
    end
  end

  describe "reconciling an existing message" do
    let!(:existing) { create(:email_message, email_account: account, provider_message_id: "m1", read: false) }

    it "flips unread → read and returns :reconciled" do
      expect(upserter.upsert(msg("status" => "1"))).to eq(:reconciled)
      expect(existing.reload.read).to be true
    end

    it "does not create a duplicate" do
      expect { upserter.upsert(msg) }.not_to change(EmailMessage, :count)
    end

    it "returns :unchanged when nothing differs" do
      expect(upserter.upsert(msg("status" => "0"))).to eq(:unchanged)
    end

    it "mirrors the Zoho flag" do
      expect(upserter.upsert(msg("flagid" => "flag_info"))).to eq(:reconciled)
      expect(existing.reload.zoho_flag).to eq("flag_info")
    end

    it "leaves a real folder id alone (provider folder moves stay deferred)" do
      original_folder = existing.provider_folder_id
      expect(upserter.upsert(msg("folderId" => "elsewhere"))).to eq(:unchanged)
      expect(existing.reload.provider_folder_id).to eq(original_folder)
    end

    context "when the existing row is a Sender-recorded placeholder" do
      let!(:existing) do
        create(:email_message, email_account: account, provider_message_id: "m1",
               read: true, provider_folder_id: "sent")
      end

      it "adopts the provider's real folder id so the Sent folder view can find it" do
        expect(upserter.upsert(msg("folderId" => "zf_sent_real", "status" => "1"))).to eq(:reconciled)
        expect(existing.reload.provider_folder_id).to eq("zf_sent_real")
      end
    end
  end

  it "skips a message with no id" do
    expect(upserter.upsert(msg("messageId" => nil))).to eq(:skipped)
  end

  describe "concurrency + resilience" do
    it "treats a unique-violation race as :created and re-queues processing" do
      existing = create(:email_message, email_account: account, provider_message_id: "m1", status: :fetched)
      messages = double("relation")
      allow(account).to receive(:email_messages).and_return(messages)
      allow(messages).to receive(:find_by).and_return(nil)
      allow(messages).to receive(:create!).and_raise(ActiveRecord::RecordNotUnique)
      allow(messages).to receive(:find_by!).and_return(existing)

      expect(described_class.new(account, scan_log: scan_log).upsert(msg)).to eq(:created)
      expect(EmailProcessJob).to have_been_enqueued.with(existing.id)
    end

    it "logs and returns :error for one bad message rather than aborting the run" do
      allow(account).to receive(:email_messages).and_raise(StandardError.new("boom"))
      expect(Rails.logger).to receive(:error).with(/boom/)
      expect(described_class.new(account, scan_log: scan_log).upsert(msg)).to eq(:error)
    end
  end

  describe "self-generated mail (our own digests / notifications)" do
    # Whatever MAILER_FROM resolves to in this environment — keeps the test honest
    # against the same source the detector reads.
    let(:our_from) { Mail::Address.new(ApplicationMailer.default[:from]).address }

    it "flags mail from our own mailer address and still enqueues processing" do
      outcome = nil
      expect { outcome = upserter.upsert(msg("fromAddress" => our_from)) }
        .to have_enqueued_job(EmailProcessJob)
      expect(outcome).to eq(:created)
      expect(EmailMessage.last.self_generated_kind).to eq("campbooks")
    end

    it "records the 'digest' kind from the X-Campbooks-Kind header" do
      upserter.upsert(msg("fromAddress" => our_from, "header_campbooks_kind" => "digest"))
      expect(EmailMessage.last.self_generated_kind).to eq("digest")
      expect(EmailMessage.last.digest?).to be true
    end

    it "leaves ordinary third-party mail unflagged" do
      upserter.upsert(msg) # fromAddress: sender@test.com
      expect(EmailMessage.last.self_generated_kind).to be_nil
      expect(EmailMessage.last.self_generated?).to be false
    end
  end
end
