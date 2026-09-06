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
                                        id: dom_id,
                                        **@attrs) do
          chip_row
          provenance_note
        end
      end

      private

      # Stable DOM id for turbo_stream replace after ask actions.
      def dom_id
        type = @counterpart.is_a?(Person) ? "person" : "organization"
        cid  = @counterpart.respond_to?(:id) ? @counterpart.id : @counterpart.counterpart_id rescue @counterpart.id
        "stand_note_#{type}_#{cid}"
      end

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
        elsif verb == :do
          do_chips
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

      # Chips for the Do lane: an accepted ask the user owes someone.
      def do_chips
        ask = @standing.ask || {}
        ask_id = ask["id"]
        due_on = ask["due_on"].present? ? Date.parse(ask["due_on"]) : nil
        held_at = ask["held_at"].presence
        today = Date.current

        chips = []

        # Done chip — PATCH to done_ask_path
        if ask_id
          chips << ask_post_chip(:done, :primary_outline, path: helpers.done_ask_path(ask_id),
                                 method: :patch, label_key: :done_ask)
        end

        # "Day on Time" link chip when dated
        if due_on
          chips << DateLinkChip.new(date: due_on, overdue: due_on < today,
                                    url: helpers.time_path(date: due_on.iso8601),
                                    label: I18n.l(due_on, format: :short))
        end

        # Hold time chip or held-slot chip
        if held_at.present?
          # Already held: show the slot as a muted chip (not a button)
          time_label = begin
            ::People::StandCopy.fmt_held(held_at)
          rescue StandardError
            held_at.to_s
          end
          chips << HeldSlotChip.new(label: t(".chips.held_slot", when: time_label))
        elsif ask_id
          # Not yet held: offer "Hold time"
          chips << ask_post_chip(:hold_time, :primary_outline, path: helpers.hold_ask_path(ask_id),
                                 method: :post, label_key: :hold_time, spark: true)
        end

        # Ask Scout
        chips << ask_scout_chip if @standing.detail.present?

        chips
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

      # A chip that posts to an ask path with an optional spark.
      def ask_post_chip(_kind, variant, path:, method: :post, label_key:, spark: false)
        token = helpers.form_authenticity_token
        label = t("components.people.stand_note.chips.#{label_key}")
        AskFormChip.new(url: path, token: token, label: label, variant: variant,
                        method: method, spark: spark, return_to_people: true)
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

      # A chip that posts to an ask path (PATCH or POST) with a return=people flag.
      class AskFormChip < Campbooks::Base
        SPARK_SVG = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[11px] w-[11px] shrink-0" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'

        def initialize(url:, token:, label:, variant: :primary, method: :post, spark: false, return_to_people: false)
          @url              = url
          @token            = token
          @label            = label
          @variant          = variant
          @method           = method
          @spark            = spark
          @return_to_people = return_to_people
        end

        def view_template
          form(action: @url, method: :post, class: "contents",
               data: { turbo_stream: true }) do
            input(type: :hidden, name: :authenticity_token, value: @token)
            input(type: :hidden, name: "_method", value: @method.to_s.upcase) if @method != :post
            input(type: :hidden, name: :return, value: "people") if @return_to_people
            render Campbooks::Button.new(type: :submit, variant: @variant, size: :sm,
                                          class: "gap-1.5") do
              raw(safe(SPARK_SVG)) if @spark
              plain @label
            end
          end
        end
      end

      # A muted non-interactive chip showing when time is already held.
      class HeldSlotChip < Campbooks::Base
        def initialize(label:)
          @label = label
        end

        def view_template
          span(class: "inline-flex items-center rounded-full border border-border/50 px-2.5 py-1 " \
                       "text-[11.5px] font-medium text-muted-foreground") do
            plain @label
          end
        end
      end

      # A link chip showing a date with optional overdue coloring.
      class DateLinkChip < Campbooks::Base
        def initialize(date:, overdue:, url:, label:)
          @date    = date
          @overdue = overdue
          @url     = url
          @label   = label
        end

        def view_template
          a(href: @url,
            data: { turbo_frame: "_top" },
            class: class_names(
              "inline-flex items-center rounded-full border px-2.5 py-1 text-[11.5px] font-medium no-underline transition-colors hover:bg-secondary",
              @overdue ? "border-red-400/50 text-red-600 dark:text-red-400" : "border-border text-foreground"
            )) do
            plain @label
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
