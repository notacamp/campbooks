# frozen_string_literal: true

module People
  # Projects the user's active home-feed items onto their People directory:
  # for each active item in the "need you" kinds, which person or organization
  # is the counterpart, and what verb does the action map to?
  #
  # This is the bridge between Now (feed items) and People (persons and orgs).
  # One People::Attention instance per request / refresh cycle.
  #
  #   attention = People::Attention.new(user, now: Time.current)
  #   attention.for(person)   # => Item | nil
  #   attention.for(org)      # => Item | nil  (only money items reach orgs)
  #   attention.participants(thread_id) # => [Person, ...]
  class Attention
    # The feed kinds that map to People attention verbs.
    KINDS = %w[reply_reminder reply_owed follow_up email_action late_receivable late_payable].freeze

    # A resolved attention item for one counterpart.
    Item = Data.define(:feed_item, :verb, :wait_days, :subject, :text, :thread_id, :message, :attention)

    def initialize(user, now: Time.current)
      @user = user
      @now = now
      @items_by_counterpart = nil
    end

    # The best attention item for a counterpart (Person or Organization), or nil.
    def for(counterpart)
      items_by_counterpart[counterpart_key(counterpart)]
    end

    # All persons whose best item is on the given thread (for group-row folding).
    def participants(thread_id)
      return [] if thread_id.blank?

      @person_items_by_thread ||= build_person_items_by_thread
      @person_items_by_thread[thread_id] || []
    end

    private

    def counterpart_key(counterpart)
      counterpart.is_a?(Person) ? [ "Person", counterpart.id ] : [ "Organization", counterpart.id ]
    end

    # Lazy-load and memoize the full projection.
    def items_by_counterpart
      @items_by_counterpart ||= build_items_by_counterpart
    end

    def build_items_by_counterpart
      feed_items = load_feed_items
      return {} if feed_items.empty?

      subjects = load_subjects(feed_items)
      sources  = {}
      result   = Hash.new { |h, k| h[k] = [] }

      feed_items.each do |fi|
        subject = subjects[[ fi.subject_type, fi.subject_id ]]
        next if subject.nil?

        source = sources[fi.kind] ||= build_source(fi.kind)
        next unless source&.still_valid?(fi, subject)

        counterpart = resolve_counterpart(fi, subject)
        next if counterpart.nil?

        item = build_item(fi, subject, counterpart)
        next if item.nil?

        key = counterpart_key(counterpart)
        result[key] << item
      end

      # Keep the best item per counterpart: highest score, then newest sort_at.
      result.transform_values do |group|
        group.max_by { |i| [ i.feed_item.score, i.feed_item.sort_at || Time.at(0) ] }
      end
    end

    def load_feed_items
      @user.feed_items.active.where(kind: KINDS).order(score: :desc).to_a
    end

    # Batch-load subjects grouped by type, with the associations People needs.
    def load_subjects(feed_items)
      grouped = feed_items.group_by(&:subject_type)
      result  = {}

      if (message_items = grouped["EmailMessage"]).present?
        ids = message_items.map(&:subject_id)
        EmailMessage.where(id: ids).includes(contact: { person: :primary_organization }, email_thread: {}).each do |m|
          result[[ "EmailMessage", m.id ]] = m
        end
      end

      if (doc_items = grouped["Document"]).present?
        ids = doc_items.map(&:subject_id)
        Document.where(id: ids).includes(email_messages: { contact: { person: :primary_organization } }).each do |d|
          result[[ "Document", d.id ]] = d
        end
      end

      result
    end

    # Returns Person or Organization to group this item under, or nil.
    def resolve_counterpart(fi, subject)
      case subject
      when EmailMessage
        subject.contact&.person
      when Document
        # Document counterpart: the org of the document's sender, else the person.
        person = document_person(subject)
        return nil if person.nil?

        person.primary_organization || person
      end
    end

    def document_person(doc)
      doc.email_messages.filter_map { |m| m.contact&.person }.first
    end

    def build_item(fi, subject, counterpart)
      verb     = verb_for(fi.kind)
      return nil if verb.nil?

      wait     = wait_days_for(fi, subject)
      subj_str = subject_string(fi, subject, counterpart)
      txt      = text_for(fi, subject)
      thread   = thread_for(fi, subject)
      msg      = subject.is_a?(EmailMessage) ? subject : nil

      Item.new(
        feed_item: fi,
        verb: verb,
        wait_days: wait,
        subject: subj_str,
        text: txt,
        thread_id: thread,
        message: msg,
        attention: fi.attention
      )
    end

    def verb_for(kind)
      case kind
      when "reply_reminder", "reply_owed" then :reply
      when "follow_up"                    then :nudge
      when "email_action"                 then :decide
      when "late_payable"                 then :pay
      when "late_receivable"              then :chase
      end
    end

    def wait_days_for(fi, subject)
      case fi.kind
      when "reply_reminder", "reply_owed", "follow_up"
        fi.data["age_days"].to_i
      when "late_payable", "late_receivable"
        fi.data["days_late"].to_i
      when "email_action"
        msg = subject.is_a?(EmailMessage) ? subject : nil
        msg&.received_at ? [ ((@now - msg.received_at) / 1.day).floor, 0 ].max : 0
      else
        0
      end
    end

    def subject_string(fi, subject, _counterpart)
      case subject
      when EmailMessage
        thread = subject.email_thread
        (thread&.display_subject.presence || subject.subject).to_s.strip
      when Document
        # Money subject: amount + due date
        cents = fi.data["amount_cents"] || subject.amount_cents
        due_raw = fi.data["due_date"] || subject.due_date&.iso8601
        amount_str = cents.present? ? ::Money.new(cents, subject.currency).format : nil
        due_str = due_raw.present? ? I18n.l(Date.parse(due_raw.to_s), format: :short) : nil

        I18n.t("people.standing.money_subject", amount: amount_str, date: due_str)
      end
    end

    def text_for(fi, subject)
      case fi.kind
      when "reply_reminder", "email_action"
        subject.is_a?(EmailMessage) ? subject.ai_action_prompt.to_s.strip.presence : nil
      when "follow_up"
        subject.is_a?(EmailMessage) ? subject.email_thread&.follow_up_reason.to_s.presence : nil
      else
        nil
      end
    end

    def thread_for(fi, subject)
      subject.is_a?(EmailMessage) ? subject.email_thread_id : nil
    end

    def build_source(kind)
      klass = Feed::Source.for_kind(kind)
      klass&.new(@user, now: @now)
    end

    # Maps thread_id → [Person, ...] for the Need-you rows, for group-row folding.
    def build_person_items_by_thread
      by_thread = Hash.new { |h, k| h[k] = [] }
      items_by_counterpart.each do |key, item|
        type, = key
        next unless type == "Person"
        next unless item.thread_id.present?

        # Recover the Person from the loaded subjects.
        fi = item.feed_item
        subj = load_single_subject(fi)
        person = subj&.contact&.person
        by_thread[item.thread_id] << [ person, item ] if person
      end
      # Sort each group by descending score.
      by_thread.transform_values { |pairs| pairs.sort_by { |_, i| -i.feed_item.score }.map(&:first) }
    end

    def load_single_subject(fi)
      return nil unless fi.subject_type == "EmailMessage"

      EmailMessage.where(id: fi.subject_id).includes(contact: :person).first
    end
  end
end
