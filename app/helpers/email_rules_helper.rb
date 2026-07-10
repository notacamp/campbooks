# frozen_string_literal: true

module EmailRulesHelper
  CATEGORY_HUMAN = {
    "notifications" => nil,  # resolved via i18n at render time
    "promotions"    => nil,
    "social"        => nil,
    "updates"       => nil
  }.freeze

  # Plain-language summary of a rule's active criteria for display in the list row.
  # Returns an HTML-safe string like:
  #   From <b>@stripe.com</b> · subject contains <b>"invoice"</b>
  def email_rule_criteria_summary(rule)
    parts = []
    c = rule.criteria

    from_vals = Array(c["from"]).reject(&:blank?)
    parts << "#{t("helpers.email_rule_criteria.from")} #{from_vals.map { |v| "<b>#{h(v)}</b>" }.join(", ")}" if from_vals.any?

    to_vals = Array(c["to"]).reject(&:blank?)
    parts << "#{t("helpers.email_rule_criteria.to")} #{to_vals.map { |v| "<b>#{h(v)}</b>" }.join(", ")}" if to_vals.any?

    subj_vals = Array(c["subject"]).reject(&:blank?)
    parts << "#{t("helpers.email_rule_criteria.subject")} #{subj_vals.map { |v| "<b>&ldquo;#{h(v)}&rdquo;</b>" }.join(", ")}" if subj_vals.any?

    body_vals = Array(c["body"]).reject(&:blank?)
    parts << "#{t("helpers.email_rule_criteria.body")} #{body_vals.map { |v| "<b>&ldquo;#{h(v)}&rdquo;</b>" }.join(", ")}" if body_vals.any?

    cat_vals = Array(c["category"]).reject(&:blank?)
    if cat_vals.any?
      human = cat_vals.map { |v| email_rule_category_human(v) }
      parts << "#{t("helpers.email_rule_criteria.category")} <b>#{h(human.join(", "))}</b>"
    end

    parts << t("helpers.email_rule_criteria.has_attachment") if c["has_attachment"] == true

    parts.any? ? parts.join(" &middot; ").html_safe : t("helpers.email_rule_criteria.no_criteria")
  end

  # Options array for the category select in the rule form.
  def email_rule_category_options
    %w[notifications promotions social updates].map do |key|
      [ email_rule_category_human(key), key ]
    end
  end

  private

  def email_rule_category_human(key)
    I18n.t("tag_groups.default_names.#{key}", default: key.humanize)
  end
end
