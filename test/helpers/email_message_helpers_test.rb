# frozen_string_literal: true

require "test_helper"

# Unit coverage for the inbox "Waiting on replies" band's recent-window collapse.
# partition_waiting_threads only reads #last_outbound_at (plus object identity),
# so a plain Struct stands in for EmailThread without touching the database.
class EmailMessageHelpersTest < ActionView::TestCase
  NOW = Time.utc(2026, 7, 5, 12, 0, 0)
  Stub = Struct.new(:id, :last_outbound_at)

  def waiting(id, days_ago)
    Stub.new(id, days_ago && (NOW - days_ago.days))
  end

  test "keeps recent threads visible and folds older ones away" do
    # Three recent threads (≥ WAITING_MIN_VISIBLE=3) → no promotion, older hidden
    threads = [ waiting(1, 2), waiting(2, 10), waiting(3, 15), waiting(4, 40), waiting(5, 90) ]
    visible, hidden = partition_waiting_threads(threads, now: NOW)
    assert_equal [ 1, 2, 3 ], visible.map(&:id)
    assert_equal [ 4, 5 ], hidden.map(&:id)
  end

  test "promotes the freshest older threads up to the minimum floor" do
    threads = [ waiting(1, 2), waiting(2, 40), waiting(3, 55), waiting(4, 90) ]
    visible, hidden = partition_waiting_threads(threads, now: NOW)
    # 1 recent + the two freshest older (40d, 55d) reaches the floor of three;
    # the 90-day thread stays hidden.
    assert_equal [ 1, 2, 3 ], visible.map(&:id)
    assert_equal [ 4 ], hidden.map(&:id)
  end

  test "shows the whole band when everything is older but under the floor" do
    threads = [ waiting(1, 40), waiting(2, 90) ]
    visible, hidden = partition_waiting_threads(threads, now: NOW)
    assert_equal [ 1, 2 ], visible.map(&:id)
    assert_empty hidden
  end

  test "hides nothing when every thread is recent" do
    threads = [ waiting(1, 1), waiting(2, 15), waiting(3, 29), waiting(4, 5) ]
    visible, hidden = partition_waiting_threads(threads, now: NOW)
    assert_equal [ 1, 2, 3, 4 ], visible.map(&:id)
    assert_empty hidden
  end

  test "treats the 30-day edge precisely and preserves incoming order" do
    inside   = Stub.new(1, NOW - 30.days + 1.minute)  # just inside the window
    edge_out = Stub.new(2, NOW - 30.days - 1.minute)  # just outside it
    # Pad with recent threads so the min-visible floor never promotes edge_out.
    pad = [ Stub.new(3, NOW - 1.day), Stub.new(4, NOW - 2.days), Stub.new(5, NOW - 3.days) ]
    visible, hidden = partition_waiting_threads([ inside, edge_out ] + pad, now: NOW)
    assert_equal [ 1, 3, 4, 5 ], visible.map(&:id)
    assert_equal [ 2 ], hidden.map(&:id)
  end

  test "handles an empty band" do
    assert_equal [ [], [] ], partition_waiting_threads([], now: NOW)
  end
end
