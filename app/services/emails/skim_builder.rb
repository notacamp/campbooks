# frozen_string_literal: true

require "date"
require "set"

module Emails
  # Groups an inbox into Skim's "rings" for the stories viewer.
  #
  # A ring is a THEME (what the mail is about) — People, Alerts, Updates,
  # Notifications, Promotions, Social — plus a leading "Priority" ring for mail the
  # user has pinned. The menu/tray shows these themes. Opening a theme walks its
  # mail by TIME: Today → Yesterday → This week → Earlier. So the structure is
  # "themes in the menu, time within a theme".
  #
  # Within a theme, similar mail collapses into cluster cards by conversation
  # thread, or by sender domain + a normalised subject topic ("GitHub PR reviews"
  # vs "GitHub security alerts" vs "47 CircleCI builds").
  #
  # Importance is a *soft* signal only: it orders cards inside a time bucket and
  # raises a confirmable "suggested priority" cue on recent mail. It is never
  # asserted as structure — the user distrusts auto-"important".
  #
  # Free + deterministic: rules categorisation (Emails::Categorizer) + plain date
  # math + group-by. No embeddings or LLM at build time. Kept dependency-light and
  # accepts an injectable `now:` so it unit-tests in isolation.
  class SkimBuilder
    # Cap the in-card expanded list so a huge cluster doesn't bloat the DOM.
    MAX_DETAIL = 20

    # Theme rings, most-actionable first. :priority holds pinned mail (any age).
    # :starred (starred-sender mail) and :pending (whitelist gatekeeping) are
    # handled explicitly in #rings and intentionally not in THEME_ORDER (which
    # only enumerates the rules-categorizer categories).
    THEME_ORDER = %i[ priority personal important updates notifications social promotions unknown ].freeze
    THEME_LABELS = {
      follow_ups:    "Follow-ups",
      starred:       "Starred",
      priority:      "Priority",
      pending:       "Pending",
      personal:      "People",
      important:     "Alerts",
      updates:       "Updates",
      notifications: "Notifications",
      social:        "Social",
      promotions:    "Promotions",
      unknown:       "Other"
    }.freeze

    # Time order walked inside a theme.
    TIME_BUCKETS = %i[ today yesterday this_week earlier ].freeze
    TIME_LABELS = {
      today:     "Today",
      yesterday: "Yesterday",
      this_week: "This week",
      earlier:   "Earlier"
    }.freeze

    # Per-theme "so what" line; secondary to the structured cues on the card.
    SUMMARIES = {
      follow_ups:    "You replied — no answer yet. Nudge them?",
      starred:       "From a sender you've starred.",
      pending:       "A new sender — allow or deny them.",
      important:     "Looks time-sensitive — worth a look.",
      # :personal is the residual bucket — mail that matched no bulk / automated /
      # social / alert signal. That's only weak evidence of a human, so the copy
      # hedges rather than asserting "a real person" (which over-claims whenever a
      # novel automated sender slips through the rules).
      personal:      "Not bulk or automated — likely personal.",
      notifications: "Routine notifications — nothing here needs you.",
      promotions:    "Marketing and newsletters. Safe to clear.",
      social:        "Social updates. Safe to clear.",
      updates:       "Order and status updates."
    }.freeze

    # Tiny stop-word list (EN + PT) so the topic key keeps the meaningful words.
    STOPWORDS = %w[
      the a an your you our for and with from this that have has are was
      re fwd fw enc res to of in on at is it be
      o a os as de da do dos das para por com sua seu seus suas um uma no na
    ].freeze

    # `memory` is an optional Emails::SkimActionMemory (or any object answering
    # #suggestion_for) that turns the user's past decisions into a per-card "you
    # usually archive these" suggestion. Left nil — and thus a no-op — so the builder
    # stays pure and unit-testable without a user / database.
    # `follow_up_thread_ids` is a Set of EmailThread ids the AI flagged as awaiting a
    # reply whose follow-up has come due (computed by Emails::SkimDeck — the builder
    # does no DB work). Their cards lead in a "Follow-ups" ring. `follow_up_meta`
    # maps thread id → { reason:, at: } for the card's "why / how long" line.
    def initialize(emails, now: Time.now, whitelist_mode: false, memory: nil,
                   follow_up_thread_ids: nil, follow_up_meta: nil)
      @emails = emails.to_a
      @today = now.to_date
      @whitelist_mode = whitelist_mode
      @memory = memory
      @follow_up_thread_ids = follow_up_thread_ids || Set.new
      @follow_up_meta = follow_up_meta || {}
    end

    # Skim-as-Stories: clusters grouped into THEME rings. Each ring's clusters are
    # ordered by time (today → earlier) so opening a theme walks it newest-first.
    # Pinned mail forms a leading "Priority" ring regardless of theme/age.
    def rings
      # Starred senders lead — promoted above everything and never grouped (each
      # email its own card, see #cluster_key). Then pinned (Priority). In whitelist
      # mode, undecided senders form a Pending ring for an allow/deny decision.
      # Blocked senders never appear (their mail is auto-archived; this also
      # covers the brief window before that archive lands).
      emails = @emails.reject { |email| blocked_sender?(email) }
      # Follow-ups lead: conversations you replied to and are still waiting to hear
      # back on, pulled out before any other grouping (they matter more than theme).
      follow_ups, emails = emails.partition { |email| follow_up?(email) }
      starred, after_starred = emails.partition { |email| starred_sender?(email) }
      pending, remaining = if @whitelist_mode
        after_starred.partition { |email| pending_sender?(email) }
      else
        [ [], after_starred ]
      end
      pinned, rest = remaining.partition { |email| pinned?(email) }
      grouped = rest.group_by { |email| category_of(email) }

      built = []
      built << build_ring(:follow_ups, follow_ups) if follow_ups.any?
      built << build_ring(:starred, starred) if starred.any?
      built << build_ring(:priority, pinned) if pinned.any?
      built << build_ring(:pending, pending) if pending.any?
      (THEME_ORDER - [ :priority ]).each do |theme|
        rows = grouped[theme]
        built << build_ring(theme, rows) if rows && rows.any?
      end
      # Any categories outside THEME_ORDER (defensive) land in a trailing "Other".
      extra = grouped.keys - THEME_ORDER
      built << build_ring(:unknown, extra.flat_map { |k| grouped[k] }) if extra.any?
      built
    end

    # Flat, globally-ordered list of cluster cards (legacy / non-Stories callers).
    def clusters
      ordered = rings.flat_map { |ring| ring[:clusters] }
                     .map { |c| c.reject { |k, _| %i[position total].include?(k) } }
      total = ordered.size
      ordered.each_with_index.map { |card, i| card.merge(position: i + 1, total: total) }
    end

    private

    # Themes where a learned keep/archive/promote suggestion doesn't belong: the
    # user already elevated Priority/Starred mail (suggesting "archive" there fights
    # them), and Pending is its own allow/deny decision.
    NO_SUGGESTION_THEMES = %i[follow_ups priority starred pending].freeze

    def build_ring(theme, rows)
      cards = order_cards(theme, build_cards(rows))
      cards.each { |card| card[:scout_suggestion] = nil } if NO_SUGGESTION_THEMES.include?(theme)
      total = cards.size

      {
        theme: theme,
        label: THEME_LABELS[theme] || theme.to_s.humanize,
        # The ring badge counts skim STEPS (one per cluster card), not the emails
        # inside them — a single 8-email cluster is one stack to skim, so it reads
        # "1", not "8". (The card itself still shows its own "8 from X" via card[:count].)
        count: total,
        stacks: total,
        senders: ring_senders(cards),
        clusters: cards.each_with_index.map { |card, i| card.merge(position: i + 1, total: total) }
      }
    end

    def build_cards(rows)
      rows.group_by { |email| cluster_key(email) }
          .map { |_key, emails| build_card(emails) }
    end

    # Starred senders lead their own ring and never fold in with other senders, but
    # a starred *conversation* still collapses to one card — otherwise a thread with
    # two starred senders (or two starred messages) spawned a duplicate card each.
    # Threadless starred mail stays one card per email. Otherwise: same conversation
    # → one card; else sender domain + normalised topic so a sender splits by topic.
    def cluster_key(email)
      thread_id = read_attr(email, :email_thread_id)

      if starred_sender?(email)
        return thread_id ? [ :starred_thread, thread_id ] : [ :starred, read_attr(email, :id) ]
      end

      return [ :thread, thread_id ] if thread_id

      [ sender_domain(email), topic_key(email) ]
    end

    # Priority is newest-first; a theme walks Today → Yesterday → This week →
    # Earlier, then by importance and recency inside each time bucket.
    def order_cards(theme, cards)
      # Follow-ups: soonest-due (longest-waiting) first, so the most overdue nudge leads.
      return cards.sort_by { |card| [ card[:follow_up_at]&.to_i || 0, -recency(card) ] } if theme == :follow_ups
      return cards.sort_by { |card| -recency(card) } if theme == :priority

      cards.sort_by { |card| [ TIME_BUCKETS.index(card[:bucket]) || 99, -card[:importance], -recency(card) ] }
    end

    def recency(card)
      card[:latest_received_at]&.to_i || 0
    end

    def build_card(emails)
      count = emails.size
      subjects = emails.map { |e| clean_subject(e) }.reject(&:empty?).uniq
      category = card_category(emails)
      ordered = emails.sort_by { |e| -(received_at(e)&.to_i || 0) }
      rep = ordered.first
      latest = ordered.map { |e| received_at(e) }.compact.max
      bucket = time_bucket(latest)
      pinned = emails.any? { |e| pinned?(e) }
      importance = emails.map { |e| importance(e) }.max || 0
      follow_up = follow_up_for(emails)

      {
        category: category,
        count: count,
        title: count == 1 ? (subjects.first || display_name(emails.first)) : "#{count} from #{display_name(emails.first)}",
        summary: card_summary(rep, count, category),
        samples: count > 1 ? subjects.first(3) : [],
        latest_received_at: latest,
        bucket: bucket,
        bucket_label: TIME_LABELS[bucket],
        unread_count: emails.count { |e| unread?(e) },
        importance: importance,
        pinned: pinned,
        priority_suggested: !pinned && %i[today yesterday].include?(bucket) && importance >= 4,
        scout_suggestion: scout_suggestion_for(rep, category),
        follow_up: follow_up.present?,
        follow_up_reason: follow_up && follow_up[:reason],
        follow_up_at: follow_up && follow_up[:at],
        email_ids: emails.map(&:id),
        emails: ordered.first(MAX_DETAIL).map { |e| email_detail(e) }
      }
    end

    # This cluster's follow-up metadata ({ reason:, at: }) if any of its emails
    # belongs to a follow-up-due thread, else nil. Cards collapse by thread, so at
    # most one thread id matches.
    def follow_up_for(emails)
      tid = emails.filter_map { |e| read_attr(e, :email_thread_id) }
                  .find { |id| @follow_up_thread_ids.include?(id) }
      tid && @follow_up_meta[tid]
    end

    def follow_up?(email)
      tid = read_attr(email, :email_thread_id)
      !tid.nil? && @follow_up_thread_ids.include?(tid)
    end

    # The learned "you usually X these" suggestion for this cluster, keyed on the
    # representative (newest) email — the same email SkimDecisionRecorder logs the
    # decision against, so record-time and lookup-time signatures agree. Returns the
    # memory's Hash ({ action:, count:, total: }) or nil.
    # A single email leads with its own AI summary when Scout has written one (the
    # real "what this is about"); clusters and un-analysed mail fall back to the
    # per-theme line. Multi-email clusters get an AI summary asynchronously via
    # Emails::SkimSummaries, which overwrites this fallback once it's cached.
    def card_summary(rep, count, category)
      ai = read_attr(rep, :ai_summary).to_s.strip if count == 1
      ai.present? ? ai : (SUMMARIES[category] || "#{count} emails.")
    end

    def scout_suggestion_for(rep, category)
      return nil unless rep && @memory

      @memory.suggestion_for(
        contact_id: read_attr(rep, :contact_id),
        sender_domain: sender_domain(rep),
        category: category
      )
    end

    def email_detail(email)
      {
        id: email.id,
        sender: display_name(email),
        subject: clean_subject(email),
        snippet: snippet_of(email),
        received_at: received_at(email)
      }
    end

    def time_bucket(time)
      return :earlier if time.nil?

      date = time.to_date
      if date == @today then :today
      elsif date == @today - 1 then :yesterday
      elsif date >= @today - 6 then :this_week
      else :earlier
      end
    end

    # 5 = starred sender (top), 4 = security / VIP (suggestable), 3 = important,
    # 2 = personal (+1 unread), 1 = updates, 0 = noise. A sort + suggestion signal.
    def importance(email)
      return 5 if starred_sender?(email)
      return 4 if subject(email).match?(Emails::Categorizer::SECURITY_SUBJECT)
      return 4 if vip?(email)

      case category_of(email)
      when :important then 3
      when :personal  then unread?(email) ? 3 : 2
      when :updates   then 1
      else 0
      end
    end

    def vip?(email)
      relationship = read_attr(read_attr(email, :contact), :relationship_type)
      %w[ client partner colleague ].include?(relationship)
    end

    def starred_sender?(email)
      !read_attr(read_attr(email, :contact), :starred_at).nil?
    end

    # Whitelist mode only: a sender we haven't admitted yet. That's an unknown
    # sender (no contact tracked) or one with no allow/deny decision (neutral,
    # not starred). Their mail waits in the Pending ring.
    def pending_sender?(email)
      contact = read_attr(email, :contact)
      return true if contact.nil?

      contact.respond_to?(:pending?) && contact.pending?
    end

    def blocked_sender?(email)
      contact = read_attr(email, :contact)
      contact.respond_to?(:blocked?) && contact.blocked?
    end

    def card_category(emails)
      cats = emails.map { |e| category_of(e) }
      THEME_ORDER.find { |c| cats.include?(c) } || cats.first
    end

    def category_of(email)
      Emails::Categorizer.new(email).call.category
    end

    # A few representative sender names for the ring's stacked avatars.
    def ring_senders(cards)
      cards.filter_map { |card| card[:emails]&.first&.dig(:sender) }.uniq.first(3)
    end

    # Normalised subject stem: strip reply/forward prefixes, [bracketed] tags,
    # numbers/ids and punctuation, drop stop-words and single chars, keep the
    # leading few significant words. Splits one sender into its real topics.
    def topic_key(email)
      stem = subject(email).downcase
      stem = stem.sub(/\A\s*(re|fwd?|fw|enc|res)\s*:\s*/i, "")
      stem = stem.gsub(/\[[^\]]*\]/, " ")
      stem = stem.gsub(/[0-9#]+/, " ")
      stem = stem.gsub(/[^[:alpha:]\s]/, " ")
      words = stem.split(/\s+/).reject { |w| w.length < 2 || STOPWORDS.include?(w) }
      words.first(4).join(" ")
    end

    def display_name(email)
      from = email.from_address.to_s
      if (match = from.match(/\A\s*"?([^"<@]+?)"?\s*</)) && !match[1].strip.empty?
        match[1].strip
      else
        root = sender_domain(email).split(".").first.to_s
        root.empty? ? from : root.capitalize
      end
    end

    def sender_domain(email)
      Emails::SenderDomain.for(email.from_address)
    end

    def clean_subject(email)
      subject(email).sub(/\A(re|fwd?|fw)\s*:\s*/i, "").strip
    end

    def snippet_of(email)
      return "" unless email.respond_to?(:summary)
      email.summary.to_s.gsub(/\s+/, " ").strip[0, 280]
    end

    # --- dependency-light attribute readers (work on AR records and test Structs)

    def subject(email) = email.subject.to_s

    def received_at(email) = read_attr(email, :received_at)

    def pinned?(email) = !read_attr(email, :pinned_at).nil?

    # `read` boolean column defaults false; treat a true value as "seen".
    def unread?(email) = read_attr(email, :read) == false

    def read_attr(object, name)
      object.respond_to?(name) ? object.public_send(name) : nil
    end
  end
end
