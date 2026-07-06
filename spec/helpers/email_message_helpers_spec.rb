# frozen_string_literal: true

require "rails_helper"

# Unit coverage for the inbox "Waiting on replies" band's recent-window collapse.
# partition_waiting_threads only reads #last_outbound_at (plus object identity),
# so a plain Struct stands in for EmailThread without touching the database.
RSpec.describe EmailMessageHelpers, type: :helper do
  let(:now) { Time.utc(2026, 7, 5, 12, 0, 0) }
  let(:stub_class) { Struct.new(:id, :last_outbound_at) }

  def waiting(id, days_ago)
    stub_class.new(id, days_ago && (now - days_ago.days))
  end

  it "keeps recent threads visible and folds older ones away" do
    # Three recent threads (>= WAITING_MIN_VISIBLE=3) → no promotion, older hidden
    threads = [ waiting(1, 2), waiting(2, 10), waiting(3, 15), waiting(4, 40), waiting(5, 90) ]
    visible, hidden = helper.partition_waiting_threads(threads, now: now)
    expect(visible.map(&:id)).to eq([ 1, 2, 3 ])
    expect(hidden.map(&:id)).to eq([ 4, 5 ])
  end

  it "promotes the freshest older threads up to the minimum floor" do
    threads = [ waiting(1, 2), waiting(2, 40), waiting(3, 55), waiting(4, 90) ]
    visible, hidden = helper.partition_waiting_threads(threads, now: now)
    # 1 recent + the two freshest older (40d, 55d) reaches the floor of three;
    # the 90-day thread stays hidden.
    expect(visible.map(&:id)).to eq([ 1, 2, 3 ])
    expect(hidden.map(&:id)).to eq([ 4 ])
  end

  it "shows the whole band when everything is older but under the floor" do
    threads = [ waiting(1, 40), waiting(2, 90) ]
    visible, hidden = helper.partition_waiting_threads(threads, now: now)
    expect(visible.map(&:id)).to eq([ 1, 2 ])
    expect(hidden).to be_empty
  end

  it "hides nothing when every thread is recent" do
    threads = [ waiting(1, 1), waiting(2, 15), waiting(3, 29), waiting(4, 5) ]
    visible, hidden = helper.partition_waiting_threads(threads, now: now)
    expect(visible.map(&:id)).to eq([ 1, 2, 3, 4 ])
    expect(hidden).to be_empty
  end

  it "treats the 30-day edge precisely and preserves incoming order" do
    inside   = stub_class.new(1, now - 30.days + 1.minute)  # just inside the window
    edge_out = stub_class.new(2, now - 30.days - 1.minute)  # just outside it
    # Pad with recent threads so the min-visible floor never promotes edge_out.
    pad = [ stub_class.new(3, now - 1.day), stub_class.new(4, now - 2.days), stub_class.new(5, now - 3.days) ]
    visible, hidden = helper.partition_waiting_threads([ inside, edge_out ] + pad, now: now)
    expect(visible.map(&:id)).to eq([ 1, 3, 4, 5 ])
    expect(hidden.map(&:id)).to eq([ 2 ])
  end

  it "handles an empty band" do
    expect(helper.partition_waiting_threads([], now: now)).to eq([ [], [] ])
  end
end
