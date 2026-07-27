require "rails_helper"

RSpec.describe Emails::InboxFolders do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let!(:inbox)    { create(:email_message, email_account: account, provider_folder_id: "INBOX") }
  let!(:archived) { create(:email_message, email_account: account, provider_folder_id: "ARCHIVE") }

  describe ".constrain" do
    let(:scope) { EmailMessage.where(email_account: account) }

    it "filters the scope to the resolved inbox folder ids (drops archived mail)" do
      allow(described_class).to receive(:ids_for).with([ account ]).and_return([ "INBOX" ])

      expect(described_class.constrain(scope, [ account ])).to contain_exactly(inbox)
    end

    it "fails open (applies no filter) when no inbox ids resolve" do
      allow(described_class).to receive(:ids_for).and_return([])

      expect(described_class.constrain(scope, [ account ])).to contain_exactly(inbox, archived)
    end
  end

  describe ".ids_for caching" do
    # The test environment uses NullStore, so we swap in a real MemoryStore for
    # the caching examples to exercise the write/read paths.
    around do |example|
      original = Rails.cache
      Rails.cache = ActiveSupport::Cache::MemoryStore.new
      example.run
    ensure
      Rails.cache = original
    end

    let(:cache_key) { "skim/inbox_folder_ids/#{account.id}" }

    it "does not cache an empty resolution so the next call re-resolves" do
      # First call returns nothing (simulates a transient provider failure).
      allow(described_class).to receive(:inbox_ids).and_return([])
      first = described_class.ids_for([ account ])
      expect(first).to eq([])
      expect(Rails.cache.read(cache_key)).to be_nil

      # Second call succeeds — must hit inbox_ids again (not a cached empty).
      allow(described_class).to receive(:inbox_ids).and_return([ "INBOX" ])
      second = described_class.ids_for([ account ])
      expect(second).to eq([ "INBOX" ])
    end

    it "caches a successful (non-empty) resolution so subsequent calls skip inbox_ids" do
      allow(described_class).to receive(:inbox_ids).and_return([ "INBOX" ]).once
      described_class.ids_for([ account ])

      # Second call must use the cache — inbox_ids is not called again.
      expect(described_class).not_to receive(:inbox_ids)
      result = described_class.ids_for([ account ])
      expect(result).to eq([ "INBOX" ])
    end
  end
end
