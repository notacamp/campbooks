require "rails_helper"

RSpec.describe Emails::SkimSummaryJob do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:cache) { ActiveSupport::Cache::MemoryStore.new }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Rails).to receive(:cache).and_return(cache)
  end

  let(:emails) { create_list(:email_message, 2, email_account: account) }
  let(:ids) { emails.map(&:id) }
  let(:digest) { Emails::SkimSummaries.digest_for(ids) }
  let(:key) { Emails::SkimSummaries.cache_key(digest) }

  it "generates, caches, and broadcasts the cluster summary" do
    allow_any_instance_of(Ai::SkimClusterSummarizer).to receive(:summary).and_return("Two routine notifications.")
    expect(Turbo::StreamsChannel).to receive(:broadcast_replace_to)
      .with("skim_#{user.id}", hash_including(target: Campbooks::SkimSummary.dom_id(digest)))

    described_class.perform_now(user.id, ids.map(&:to_s), digest)

    expect(cache.read(key)).to eq("Two routine notifications.")
  end

  it "is idempotent — does no work when the summary is already cached" do
    cache.write(key, "already")
    expect_any_instance_of(Ai::SkimClusterSummarizer).not_to receive(:summary)

    described_class.perform_now(user.id, ids.map(&:to_s), digest)

    expect(cache.read(key)).to eq("already")
  end

  it "caches nothing when the summarizer returns nil (no model / failure)" do
    allow_any_instance_of(Ai::SkimClusterSummarizer).to receive(:summary).and_return(nil)

    described_class.perform_now(user.id, ids.map(&:to_s), digest)

    expect(cache.read(key)).to be_nil
  end

  it "never summarizes mail the user cannot read" do
    foreign = create(:email_message, email_account: create(:email_account, workspace: create(:workspace)))
    expect_any_instance_of(Ai::SkimClusterSummarizer).not_to receive(:summary)

    described_class.perform_now(user.id, [ foreign.id.to_s ], Emails::SkimSummaries.digest_for([ foreign.id ]))
  end

  it "leaves Current.workspace clean afterwards" do
    allow_any_instance_of(Ai::SkimClusterSummarizer).to receive(:summary).and_return("x")
    allow(Turbo::StreamsChannel).to receive(:broadcast_replace_to)

    described_class.perform_now(user.id, ids.map(&:to_s), digest)

    expect(Current.workspace).to be_nil
  end
end
