# frozen_string_literal: true

module Feed
  # "Rewind": the home feed's scroll-back through PAST HIGHLIGHTS. Once the
  # curated, actionable spine runs out, the same infinite scroll keeps surfacing
  # the standout emails from progressively older periods — never a raw list of
  # every message (that's the inbox the product exists to escape).
  #
  # An email is a highlight if it carries a signal worth resurfacing:
  #   • from a STARRED sender
  #   • Scout flagged it IMPORTANT or HIGH priority
  #   • it carries an ATTACHMENT (invoice / contract / doc) and isn't bulk
  #   • it lives in a BUSY thread (a real, long conversation) and isn't bulk
  # Everything else — promotions, notifications, social, plain receipts, OTP
  # codes — is noise and never appears. On real data this keeps ~10% of mail,
  # with standouts in every year back to the first message.
  #
  # Results stream newest -> oldest, keyset-paginated (COALESCE(received_at,
  # created_at), id) so nothing is materialized and an old page costs a query
  # only when the user actually scrolls to it. Cards are grouped into time
  # CHAPTERS ("This month", "March", "2024") that the cursor carries forward so a
  # chapter spanning a page boundary isn't repeated.
  class Rewind
    PAGE_SIZE = 8

    # Categories Scout files as bulk/low-signal — excluded unless a strong signal
    # (starred sender, important) overrides. Tuned against real inbox data, where
    # these three are ~64% of all mail.
    NOISE_CATEGORIES = %w[promotions notifications social updates].freeze

    # A thread needs this many messages to count as a "busy" conversation. Tuned
    # so busy-thread doesn't swamp the set (at >=5 it tagged ~29% of mail).
    BUSY_THREAD_MIN = 8

    # A keyset position + the chapter open at that point, so the next page knows
    # whether its first card starts a new chapter.
    Cursor = Struct.new(:sort_at, :id, :period)

    SORT = "COALESCE(email_messages.received_at, email_messages.created_at)"
    ORDER = Arel.sql("#{SORT} DESC, email_messages.id DESC")

    def self.cursor_from_params(params)
      before = params[:before]
      before_id = params[:before_id]
      return nil if before.blank? || before_id.blank?

      at = Time.zone.parse(before.to_s)
      return nil if at.nil?

      Cursor.new(at, before_id.to_s, params[:period].presence)
    rescue ArgumentError, TypeError
      nil
    end

    def initialize(user, before: nil, now: Time.current)
      @user = user
      @before = before
      @now = now
    end

    # Any past highlight to show at all? Drives whether the feed hands off to the
    # rewind or just ends. One cheap EXISTS.
    def any?
      base_scope.exists?
    end

    # The page's render list: chapter markers interleaved with highlight cards.
    # A chapter is emitted before the first card of each new period, seeded with
    # the chapter the previous page ended in (so it isn't repeated mid-chapter).
    # ⇒ [{type: :chapter, key:, label:, count:}, {type: :card, email:}, …]
    def entries
      running = @before&.period
      page.flat_map do |email|
        key = period_key(email)
        next [ card_entry(email) ] if key == running

        running = key
        [ chapter_entry(email, key), card_entry(email) ]
      end
    end

    # Cursor for the next page, or nil at the end of the highlights.
    def next_cursor
      return nil if page.size < PAGE_SIZE

      last = page.last
      Cursor.new(last.received_at || last.created_at, last.id, period_key(last))
    end

    # The highlight emails on this page (PAGE_SIZE), newest first. #entries wraps
    # them with chapter markers; the raw list is handy for callers and tests.
    def page
      @page ||= cursor_scope.limit(PAGE_SIZE).to_a
    end

    private

    def card_entry(email) = { type: :card, email: email, reason: reason_for(email) }

    # The dominant signal that earned this email its place — the card leads with
    # it ("why am I seeing this?"). Strong signals win: starred > important >
    # high priority > attachment > busy thread. Cheap in-memory checks against the
    # already-loaded id sets, no extra queries.
    def reason_for(email)
      return :starred       if starred_set.include?(email.contact_id)
      return :important     if email.category == "important"
      return :high_priority if email.ai_priority == "high"
      return :attachment    if email.has_attachment?
      return :busy_thread   if email.email_thread_id && busy_set.include?(email.email_thread_id)

      :starred # unreachable: every page row matched the filter
    end

    def chapter_entry(email, key)
      { type: :chapter, key: key, label: period_label(email), count: period_count(email) }
    end

    # Highlights the user may see, minus anything already shown as a curated card
    # above, newest first.
    def base_scope
      highlight_scope
        .where.not(id: curated_email_ids)
        .reorder(ORDER)
    end

    def cursor_scope
      scope = base_scope.includes(:email_account, :tags, :email_thread, contact: :sender_tags)
      return scope if @before.nil?

      scope.where(
        "#{SORT} < :at OR (#{SORT} = :at AND email_messages.id < :id)",
        at: @before.sort_at, id: @before.id
      )
    end

    # The highlight filter (see class doc). Strong signals (starred, important,
    # high) stand alone; attachment / busy-thread are gated on not-bulk.
    #
    # Constrained to the inbox folders (Emails::InboxFolders — the same gate
    # Feed::Source#in_inbox and Skim use) so archived mail drops out. Without it,
    # archiving a Rewind card removed it from view but it reappeared on the next
    # lazy page / reload, since it still matched this signal filter — i.e. archive
    # didn't stick in "Looking back". Fails open (no constraint) when folder ids
    # can't be resolved, matching the rest of the feed.
    def highlight_scope
      Emails::InboxFolders.constrain(
        EmailMessage.accessible_to(@user).where(
          "email_messages.contact_id IN (:starred) " \
          "OR email_messages.category = :important " \
          "OR email_messages.ai_priority = :high " \
          "OR (email_messages.has_attachment = TRUE AND (email_messages.category IS NULL OR email_messages.category NOT IN (:noise))) " \
          "OR (email_messages.email_thread_id IN (:busy) AND (email_messages.category IS NULL OR email_messages.category NOT IN (:noise)))",
          starred: starred_contact_ids,
          important: "important",
          high: EmailMessage.ai_priorities[:high],
          noise: NOISE_CATEGORIES,
          busy: busy_thread_ids
        ),
        readable_accounts
      )
    end

    # Total highlights in the chapter `email` opens — the chapter's "N highlights"
    # badge. One bounded count per chapter (chapters are months/years, few).
    def period_count(email)
      from, to = period_bounds(effective_date(email))
      highlight_scope.where.not(id: curated_email_ids)
                     .where("#{SORT} BETWEEN ? AND ?", from, to).count
    end

    # --- period (chapter) helpers ---------------------------------------------

    def effective_date(email) = (email.received_at || email.created_at).to_date

    # Stable identity for a date's chapter: "current" (this month), "YYYY-MM" (an
    # earlier month this year), or "YYYY" (a past year).
    def period_key(email)
      d = effective_date(email)
      if d.year == today.year && d.month == today.month then "current"
      elsif d.year == today.year then format("%<y>d-%<m>02d", y: d.year, m: d.month)
      else d.year.to_s
      end
    end

    def period_label(email)
      d = effective_date(email)
      if d.year == today.year && d.month == today.month
        I18n.t("home.index.rewind_this_month")
      elsif d.year == today.year
        I18n.l(d, format: "%B") # localized full month name (rails-i18n month_names)
      else
        d.year.to_s
      end
    end

    def period_bounds(date)
      if date.year == today.year && date.month == today.month
        [ today.beginning_of_month.beginning_of_day, @now ]
      elsif date.year == today.year
        [ date.beginning_of_month.beginning_of_day, date.end_of_month.end_of_day ]
      else
        [ Date.new(date.year, 1, 1).beginning_of_day, Date.new(date.year, 12, 31).end_of_day ]
      end
    end

    def today = @now.to_date

    # --- signal sets ----------------------------------------------------------

    def starred_contact_ids
      @starred_contact_ids ||=
        Contact.where(workspace_id: @user.workspace_id).where.not(starred_at: nil).pluck(:id)
    end

    # O(1) membership for the per-card reason check.
    def starred_set = @starred_set ||= starred_contact_ids.to_set
    def busy_set    = @busy_set ||= busy_thread_ids.to_set

    # Busy-thread ids need a GROUP BY ... HAVING over the whole mailbox, too dear
    # to run on every lazy page — cache the small id set briefly (highlights need
    # not be real-time fresh).
    def busy_thread_ids
      @busy_thread_ids ||= Rails.cache.fetch(busy_cache_key, expires_in: 10.minutes) do
        EmailMessage.where(email_account_id: readable_account_ids)
                    .where.not(email_thread_id: nil)
                    .group(:email_thread_id)
                    .having("COUNT(*) >= ?", BUSY_THREAD_MIN)
                    .count.keys
      end
    end

    def curated_email_ids
      @curated_email_ids ||=
        @user.feed_items.active.where(subject_type: "EmailMessage").pluck(:subject_id)
    end

    # Full account records (not just ids): Emails::InboxFolders.constrain needs
    # them to resolve each account's inbox folder via its mail client.
    def readable_accounts
      @readable_accounts ||= @user.readable_email_accounts.to_a
    end

    def readable_account_ids
      @readable_account_ids ||= readable_accounts.map(&:id).sort
    end

    def busy_cache_key
      "feed/rewind/busy_threads/user_#{@user.id}/#{readable_account_ids.join('-')}"
    end
  end
end
