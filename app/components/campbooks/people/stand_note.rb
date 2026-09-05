# frozen_string_literal: true

module Campbooks
  module People
    # Scout's "where things stand" for one counterpart: the sentence (People::StandCopy.note)
    # inside the Ember-glass ScoutNote, with the actions that resolve it. Replaces the
    # generic EmailScoutActions strip on the People conversation and organization pages.
    #
    # @param standing [People::Standing::Result]
    # @param counterpart [Person, Organization] the live record or a People::Counterpart
    # @param reply_target [EmailMessage, nil] the message to reply to / follow up on
    # @param can_send [Boolean] whether the current user can send email
    # @param draft_present [Boolean] whether Scout has already drafted a reply below
    # @param surface [Symbol] passed through to the draft-reply tool call
    class StandNote < Campbooks::Base
      def initialize(standing:, counterpart:, reply_target: nil, can_send: false,
                     draft_present: false, surface: :detail, **attrs)
        @standing      = standing
        @counterpart   = counterpart
        @reply_target  = reply_target
        @can_send      = can_send
        @draft_present = draft_present
        @surface       = surface
        @attrs         = attrs
      end

      def view_template
        render Campbooks::ScoutNote.new(message: note_message,
                                        time: t("people.conversation.where_things_stand"),
                                        **@attrs) do
          chip_row
          provenance_note
        end
      end

      private

      def note_message
        name = counterpart_name
        base = ::People::StandCopy.note(@standing, name: name, date: @reply_target&.received_at)
        base ||= t("people.conversation.no_standing")
        if @draft_present && @standing.verb == :reply
          "#{base} #{t("people.conversation.stand.draft_below")}"
        else
          base
        end
      end

      def counterpart_name
        if @counterpart.respond_to?(:display_name)
          helpers.people_first_name(@counterpart)
        else
          @counterpart.name.to_s
        end
      end

      def chip_row
        chips = build_chips
        return unless chips.any?

        div(class: "mt-3 flex flex-wrap items-center gap-2") do
          chips.each { |chip| render chip }
        end
      end

      def build_chips
        verb = @standing.verb
        dk   = @standing.detail_kind

        if verb == :reply
          reply_chips
        elsif verb == :nudge
          nudge_chips
        elsif verb == :decide
          decide_chips(dk)
        elsif verb == :pay || verb == :chase
          money_chips
        elsif verb.nil? && dk.in?(%i[ask_ai ask_quote])
          no_verb_ask_chips
        else
          []
        end
      end

      def reply_chips
        chips = []
        if @can_send && @reply_target && !@draft_present
          chips << tool_form_chip(:draft_reply, :primary)
        end
        chips << post_chip(:done, :outline) if @standing.feed_item_id
        chips << post_chip(:snooze, :ghost, hidden: { "until" => "tomorrow" }) if @standing.email_message_id
        chips
      end

      def nudge_chips
        chips = []
        chips << tool_form_chip(:draft_follow_up, :primary) if @can_send && @reply_target
        chips << post_chip(:done, :outline, label_key: :let_it_go) if @standing.feed_item_id
        chips
      end

      def decide_chips(dk)
        if dk == :prompt
          chips = []
          chips << ask_scout_chip
          chips << tool_form_chip(:draft_reply, :outline) if @can_send && @reply_target
          chips << post_chip(:done, :ghost) if @standing.feed_item_id
          chips
        else
          reply_chips
        end
      end

      def money_chips
        chips = [ open_money_chip ]
        chips << post_chip(:paid, :outline, label_key: :mark_paid) if @standing.feed_item_id
        chips
      end

      def no_verb_ask_chips
        return [] unless @can_send && @reply_target

        [ tool_form_chip(:draft_reply, :primary) ]
      end

      # ── Individual chip factories ────────────────────────────────────────────

      def tool_form_chip(tool, variant)
        url   = helpers.tool_email_message_path(@reply_target, tool: tool, surface: @surface)
        token = helpers.form_authenticity_token
        label = t("components.people.stand_note.chips.#{tool}")
        FormChip.new(url: url, token: token, label: label, variant: variant)
      end

      def post_chip(kind, variant, label_key: nil, hidden: {})
        url   = helpers.people_action_path(@counterpart.id, kind)
        token = helpers.form_authenticity_token
        label = t("components.people.stand_note.chips.#{label_key || kind}")
        FormChip.new(url: url, token: token, label: label, variant: variant, hidden_fields: hidden)
      end

      def ask_scout_chip
        ctx  = t("components.people.stand_note.ask_scout_context",
                  subject: @standing.subject.to_s.truncate(80), name: counterpart_name)
        text = "#{ctx} #{@standing.detail}"
        AskScoutChip.new(text: text, label: t("components.people.stand_note.chips.ask_scout"))
      end

      def open_money_chip
        OpenMoneyChip.new(label: t("components.people.stand_note.chips.open_money"),
                          url: helpers.money_path)
      end

      def provenance_note
        dk = @standing.detail_kind
        return unless dk.in?(%i[ask_ai prompt])
        return unless @reply_target&.respond_to?(:ai_provenance)
        return unless @reply_target.ai_provenance&.dig("provider").present?

        div(class: "mt-2.5") do
          render Campbooks::AiProvenanceNote.new(provenance: @reply_target.ai_provenance)
        end
      end

      # ── Inner helper components ──────────────────────────────────────────────

      # A chip rendered as a small form with a POST button.
      class FormChip < Campbooks::Base
        def initialize(url:, token:, label:, variant: :primary, hidden_fields: {})
          @url           = url
          @token         = token
          @label         = label
          @variant       = variant
          @hidden_fields = hidden_fields
        end

        def view_template
          form(action: @url, method: :post, class: "contents",
               data: { turbo_stream: true }) do
            input(type: :hidden, name: :authenticity_token, value: @token)
            @hidden_fields.each do |name, value|
              input(type: :hidden, name: name, value: value)
            end
            render Campbooks::Button.new(type: :submit, variant: @variant, size: :sm) { plain @label }
          end
        end
      end

      # The "Ask Scout" chip — a plain button that fires the global scout:ask event.
      class AskScoutChip < Campbooks::Base
        def initialize(text:, label:)
          @text  = text
          @label = label
        end

        def view_template
          render Campbooks::Button.new(
            type: :button,
            variant: :primary,
            size: :sm,
            data: { controller: "scout-ask",
                    scout_ask_text_value: @text,
                    action: "click->scout-ask#fire" }
          ) { plain @label }
        end
      end

      # The "Open in Money" chip — an anchor styled as a primary button.
      class OpenMoneyChip < Campbooks::Base
        def initialize(label:, url:)
          @label = label
          @url   = url
        end

        def view_template
          render Campbooks::Button.new(href: @url, variant: :primary, size: :sm,
                                        data: { turbo_frame: "_top" }) { plain @label }
        end
      end
    end
  end
end
