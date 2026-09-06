module Contacts
  class ContactContextBuilder
    def initialize(from_address, user: nil)
      @contact = find_contact(from_address)
      @person = @contact&.person
      @user = user
    end

    def context_for_prompt
      parts = []

      if @person&.context_summary.present?
        parts << "This sender (#{@contact.email}) is #{@person.context_summary}"
        parts << "They represent #{@person.organization}." if @person.organization.present?
        parts << "Relationship: #{@person.relationship_type}." if @person.relationship_type.present?
      end

      attention_block = build_attention_block
      parts << attention_block if attention_block

      top_line = build_top_people_line
      parts << top_line if top_line

      return nil if parts.empty?

      "<contact_context>\n#{parts.join(" ")}\n</contact_context>"
    end

    private

    def resolved_user
      @resolved_user ||= @user ||
        @contact&.email_account&.email_account_users&.find_by(owner: true)&.user
    end

    def sender_weight
      return @sender_weight if defined?(@sender_weight)

      @sender_weight = if resolved_user && @person
        AttentionWeight.for_user(resolved_user).find_by(subject_type: "Person", subject_id: @person.id)
      end
    end

    def build_attention_block
      return nil unless resolved_user && @person && sender_weight

      aw = sender_weight
      reasons_text = aw.reason_values.map(&:sentence).join("; ")

      taught_verdict = LearningDecision
        .where(domain: Attention::Teach::DOMAIN, user_id: resolved_user.id,
               contact_id: @person.contacts.select(:id))
        .order(created_at: :desc)
        .first&.label

      if taught_verdict == "important"
        "The user told Scout this sender matters. #{reasons_text.presence ? "Reasons: #{reasons_text}." : ''} Lean toward a higher priority and a concrete action prompt."
      elsif taught_verdict == "unimportant"
        "The user told Scout this sender does not matter. Unless the message is clearly time-critical, keep priority low and the action prompt empty."
      elsif aw.weight >= 0.6
        "Attention: this sender matters a lot to the user (#{reasons_text}). Lean toward a higher priority and a concrete action prompt."
      elsif aw.weight <= 0.15 && aw.confidence >= 0.5
        "Attention: the user rarely engages with this sender (#{reasons_text}). Unless the message is clearly time-critical, keep priority low and the action prompt empty."
      end
    end

    def build_top_people_line
      return nil unless resolved_user

      top = AttentionWeight
        .for_user(resolved_user)
        .where(subject_type: "Person")
        .ranked
        .limit(5)

      names = top.filter_map do |aw|
        Person.find_by(id: aw.subject_id)&.display_name
      end

      return nil if names.empty?

      "People who matter most to the user: #{names.join(', ')}."
    end

    def find_contact(email_address)
      Contact.find_by(email: email_address) ||
        ContactEmailAlias.includes(:contact).find_by(email: email_address)&.contact
    end
  end
end
