# frozen_string_literal: true

module Scout
  module Memory
    # Turns a free-text "teach Scout something" sentence into a real behaviour
    # record, then hands back the memory Entry id so the page can render the new
    # sentence. Deterministic parsers run first (Scout only ever executes a bounded
    # set of intent shapes — never arbitrary changes); if none match and a text
    # model is configured, the AI mapper maps the sentence onto ONE of those same
    # shapes and the same executor runs it. Otherwise it replies "I can't learn
    # that yet".
    #
    #   result = Scout::Memory::Teacher.new(workspace:, user:).learn("treat GitHub notifications as a stream")
    #   result.created?  # => true
    #   result.entry_id  # => "group:<uuid>"
    class Teacher
      PARSERS = [
        Parsers::Stream, Parsers::FileRule, Parsers::TagRule,
        Parsers::Priority, Parsers::Block, Parsers::Signature
      ].freeze

      # The intent shapes the executor understands, and the keys each requires.
      # The AI mapper's output is validated against this before anything runs.
      SHAPES = {
        stream: %i[value group],
        file: %i[from folder],
        tag: %i[from tag],
        priority: %i[contact],
        block: %i[contact],
        signature: %i[name]
      }.freeze

      Result = Data.define(:status, :entry_id, :message) do
        def created? = status == :created
        def unknown? = status == :unknown
      end

      # Injectable: a callable (String -> intent Hash | nil). Left nil in
      # development/test (see config/initializers/scout_memory.rb) so the Teacher
      # never calls a real provider there; specs inject a stub.
      class << self
        attr_accessor :ai_mapper
      end

      def initialize(workspace:, user:)
        @workspace = workspace
        @user = user
      end

      def learn(text)
        text = text.to_s.strip
        return unknown if text.blank?

        intent = parse(text) || ai_intent(text)
        return unknown unless intent

        execute(intent)
      rescue => e
        Rails.logger.warn("[Scout::Memory::Teacher] #{e.class}: #{e.message}")
        Result.new(status: :error, entry_id: nil, message: I18n.t("scout_memory.teach.error"))
      end

      private

      def parse(text)
        PARSERS.each do |parser|
          intent = parser.call(text)
          return intent if intent
        end
        nil
      end

      def ai_intent(text)
        mapper = self.class.ai_mapper
        return nil unless mapper

        validate_intent(mapper.call(text))
      end

      def validate_intent(raw)
        return nil unless raw.is_a?(Hash)

        raw = raw.symbolize_keys
        kind = raw[:kind]&.to_sym
        required = SHAPES[kind]
        return nil unless required
        return nil unless required.all? { |key| raw[key].to_s.strip.present? }

        raw.slice(*required).merge(kind: kind)
      end

      def execute(intent)
        case intent[:kind]
        when :stream    then created("group:#{stream_rule(intent).id}")
        when :file      then created("rule:#{file_rule(intent).id}")
        when :tag       then created("rule:#{tag_rule(intent).id}")
        when :priority  then priority(intent)
        when :block     then blocking(intent)
        when :signature then signature(intent)
        else unknown
        end
      end

      def stream_rule(intent)
        @workspace.inbox_group_rules.create!(
          rule_type: "sender", value: intent[:value].to_s.downcase, group_name: intent[:group]
        )
      end

      def file_rule(intent)
        folder = find_or_create_folder(intent[:folder])
        @workspace.email_rules.create!(
          name: rule_name(intent[:from]), criteria: { "from" => [ intent[:from] ] },
          mail_folder: folder, created_by: @user
        )
      end

      def tag_rule(intent)
        tag = find_or_create_tag(intent[:tag])
        @workspace.email_rules.create!(
          name: rule_name(intent[:from]), criteria: { "from" => [ intent[:from] ] },
          tags: [ tag ], created_by: @user
        )
      end

      def priority(intent)
        contact = find_contact(intent[:contact])
        return unknown unless contact

        contact.star!
        created("starred:#{contact.id}")
      end

      def blocking(intent)
        contact = find_contact(intent[:contact])
        return unknown unless contact

        Contacts::Block.call(contact, user: @user)
        created("blocked:#{contact.id}")
      end

      def signature(intent)
        sig = @user.signatures.where("LOWER(name) = ?", intent[:name].to_s.downcase).first
        return unknown unless sig

        sig.make_default!
        created("signature:#{sig.id}")
      end

      def find_or_create_folder(name)
        @workspace.mail_folders.where("LOWER(name) = ?", name.to_s.downcase).first ||
          @workspace.mail_folders.create!(name: name, position: 0)
      end

      def find_or_create_tag(name)
        @workspace.tags.where("LOWER(name) = ?", name.to_s.downcase).first ||
          @workspace.tags.create!(name: name, color: Tag.palette_color_for(name))
      end

      # A contact must resolve to a real record. Emails find-or-create; a bare name
      # only matches an existing contact (we can't invent an address).
      def find_contact(who)
        if Scout::Memory::Parsers::Shared.email?(who)
          email = Scout::Memory::Parsers::Shared.clean(who).downcase
          @workspace.contacts.where("LOWER(email) = ?", email).first ||
            @workspace.contacts.create!(email: email)
        else
          @workspace.contacts.where("LOWER(name) = ?", who.to_s.downcase).first
        end
      end

      def rule_name(from)
        "From #{Scout::Memory::Parsers::Shared.clean(from)}".truncate(120)
      end

      def created(entry_id)
        Result.new(status: :created, entry_id: entry_id, message: nil)
      end

      def unknown
        Result.new(status: :unknown, entry_id: nil, message: I18n.t("scout_memory.teach.unknown"))
      end
    end
  end
end
