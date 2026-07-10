# frozen_string_literal: true

module EmailRules
  # Builds an EmailMessage scope that matches the given rule's criteria.
  # Job-safe: no dependence on Current.user.  Permission is enforced through
  # account-id scoping (workspace accounts) rather than per-user readable
  # accounts, consistent with the Emails::TagGroups#base_messages idiom.
  #
  # Outbound messages (emails sent FROM the workspace's own mailbox addresses)
  # are always excluded — rules act on received mail only.  The exclusion mirrors
  # EmailProcessJob#is_outbound? / EmailMessage#outbound? in SQL:
  #   NOT (LOWER(from_address) LIKE '%' || LOWER(account.email_address) || '%')
  #
  # Any-of semantics (OR) within a criterion key; AND across keys.
  # ILIKE clauses use sanitize_sql_like-escaped values — no raw interpolation.
  class Matcher
    UNDOABLE_THRESHOLD = 25_000

    def initialize(rule)
      @rule = rule
    end

    # An EmailMessage ActiveRecord relation matching the rule's criteria.
    # Memoised so repeated calls (e.g. count then iterate) share one query plan.
    def scope
      @scope ||= build_scope
    end

    # Returns true when the given email is within the rule's scope.
    def matches?(email)
      scope.where(id: email.id).exists?
    end

    # Count of currently matching messages (snapshot).
    def count
      scope.count
    end

    private

    def build_scope
      account_ids = resolve_account_ids
      return EmailMessage.none if account_ids.empty?

      # Exclude outbound mail — rules act on received mail only.
      # SQL mirrors EmailMessage#outbound? (EmailMessage#sent?):
      #   from_address.include?(email_account.email_address)  (case-insensitive substring)
      rel = EmailMessage
        .where(email_account_id: account_ids)
        .joins(:email_account)
        .where(
          "LOWER(email_messages.from_address) NOT LIKE ('%' || LOWER(email_accounts.email_address) || '%')"
        )

      apply_criteria(rel)
    end

    # Account ids for the workspace, optionally narrowed to a single account
    # when the criterion specifies email_account_id.
    def resolve_account_ids
      ws_ids = @rule.workspace.email_accounts.ids
      account_id = @rule.criteria["email_account_id"].to_s.presence
      account_id ? ws_ids & [ account_id ] : ws_ids
    end

    def apply_criteria(rel)
      rel = apply_from(rel)
      rel = apply_to(rel)
      rel = apply_subject(rel)
      rel = apply_body(rel)
      rel = apply_category(rel)
      rel = apply_has_attachment(rel)
      rel
    end

    # from: any-of — "@domain" matches any address at that domain;
    # otherwise a substring ILIKE on from_address.
    def apply_from(rel)
      values = array_criterion("from")
      return rel if values.empty?

      conditions = values.map do |v|
        if v.start_with?("@")
          domain = sanitize_like(v.delete_prefix("@"))
          safe_ilike("email_messages.from_address", "%@#{domain}%")
        else
          safe_ilike("email_messages.from_address", "%#{sanitize_like(v)}%")
        end
      end

      rel.where(conditions.join(" OR "))
    end

    # to: any-of — matches to_address OR cc_address for each value.
    def apply_to(rel)
      values = array_criterion("to")
      return rel if values.empty?

      conditions = values.map do |v|
        like = "%#{sanitize_like(v)}%"
        "(#{safe_ilike("email_messages.to_address", like)} OR #{safe_ilike("email_messages.cc_address", like)})"
      end

      rel.where(conditions.join(" OR "))
    end

    # subject: any-of ILIKE contains.
    def apply_subject(rel)
      values = array_criterion("subject")
      return rel if values.empty?

      conditions = values.map { |v| safe_ilike("email_messages.subject", "%#{sanitize_like(v)}%") }
      rel.where(conditions.join(" OR "))
    end

    # body: any-of ILIKE contains.
    # NOTE: Campbooks stores only the HTML body in the `body` column — there is
    # no separate plain-text column.  Matching against HTML may produce false
    # positives on HTML tags, but it is the only available full-body column.
    # (Deep body search via embeddings is available in meaning-mode search but
    # has no ranking context here.)
    def apply_body(rel)
      values = array_criterion("body")
      return rel if values.empty?

      conditions = values.map { |v| safe_ilike("email_messages.body", "%#{sanitize_like(v)}%") }
      rel.where(conditions.join(" OR "))
    end

    # category: any-of exact enum match.
    def apply_category(rel)
      values = array_criterion("category")
      return rel if values.empty?

      rel.where(category: values)
    end

    def apply_has_attachment(rel)
      @rule.criteria["has_attachment"] == true ? rel.where(has_attachment: true) : rel
    end

    # Return a sanitized SQL ILIKE fragment (bound via sanitize_sql_array so the
    # value is properly quoted — no raw string interpolation of user data).
    def safe_ilike(column, pattern)
      EmailMessage.sanitize_sql_array([ "#{column} ILIKE ?", pattern ])
    end

    def sanitize_like(str)
      EmailMessage.sanitize_sql_like(str.to_s)
    end

    def array_criterion(key)
      Array(@rule.criteria[key]).reject(&:blank?)
    end
  end
end
