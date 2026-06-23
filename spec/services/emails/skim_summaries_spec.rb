require "rails_helper"

RSpec.describe Emails::SkimSummaries do
  # The post-pass only needs the user's id (for the job arg) — no DB row required.
  let(:user) { instance_double(User, id: 99) }
  let(:cache) { ActiveSupport::Cache::MemoryStore.new }

  before { allow(Rails).to receive(:cache).and_return(cache) }

  def rings_with(card) = [ { theme: :notifications, clusters: [ card ] } ]

  def cluster(count:, email_ids:, summary: "fallback")
    { count: count, email_ids: email_ids, summary: summary }
  end

  describe "#digest_for" do
    it "is stable and order-independent across id types" do
      expect(described_class.digest_for([ 3, 1, 2 ])).to eq(described_class.digest_for(%w[1 2 3]))
    end

    it "changes when the member set changes" do
      expect(described_class.digest_for([ 1, 2 ])).not_to eq(described_class.digest_for([ 1, 2, 3 ]))
    end
  end

  it "applies a cached summary onto a multi-email card and stamps the digest" do
    ids = [ 1, 2, 3 ]
    digest = described_class.digest_for(ids)
    cache.write(described_class.cache_key(digest), "Three CI failures need a look.")

    rings = rings_with(cluster(count: 3, email_ids: ids))
    described_class.new(user).apply!(rings)

    card = rings.first[:clusters].first
    expect(card[:summary]).to eq("Three CI failures need a look.")
    expect(card[:summary_digest]).to eq(digest)
  end

  it "enqueues a generation job on a cache miss and leaves the fallback in place" do
    ids = [ 10, 11 ]
    expect(Emails::SkimSummaryJob).to receive(:perform_later).with(user.id, %w[10 11], described_class.digest_for(ids))

    rings = rings_with(cluster(count: 5, email_ids: ids, summary: "5 emails."))
    described_class.new(user).apply!(rings)

    expect(rings.first[:clusters].first[:summary]).to eq("5 emails.")
  end

  it "enqueues only once for the same cold cluster (dedup marker)" do
    expect(Emails::SkimSummaryJob).to receive(:perform_later).once

    described_class.new(user).apply!(rings_with(cluster(count: 2, email_ids: [ 7, 8 ])))
    described_class.new(user).apply!(rings_with(cluster(count: 2, email_ids: [ 7, 8 ])))
  end

  it "skips single-email cards (they already show their own ai_summary)" do
    expect(Emails::SkimSummaryJob).not_to receive(:perform_later)

    rings = rings_with(cluster(count: 1, email_ids: [ 9 ], summary: "single"))
    described_class.new(user).apply!(rings)

    expect(rings.first[:clusters].first[:summary]).to eq("single")
    expect(rings.first[:clusters].first[:summary_digest]).to be_nil
  end
end
