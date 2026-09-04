# frozen_string_literal: true

module Campbooks
  module People
    # The Details rail (≥ xl / 1280px) or sheet (< xl) for one person.
    # Rendered inside turbo-frame "people_details" by People::DetailsController.
    # Sections in spec order: Identity → Scout's Read → Numbers → Threads →
    # Documents → Events → Manage.
    class Details < Campbooks::Base
      SECTION_H = "pb-1 pt-5 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground"
      FACT_LABEL = "text-[12px] text-muted-foreground"
      FACT_VALUE = "text-[12px] tabular-nums text-foreground"
      ACTION_BTN = "inline-flex items-center gap-1.5 rounded-lg border border-border px-3 py-1.5 " \
                   "text-[12.5px] text-foreground hover:bg-secondary transition-colors"
      CHIP_BASE  = "inline-flex items-center rounded-full px-2 py-0.5 text-[11.5px] font-medium"

      ICON_PENCIL = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>'
      ICON_COPY   = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>'
      ICON_REFRESH = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.61"/></svg>'
      ICON_MONEY  = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4"><path d="M12 1v22M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/></svg>'
      ICON_FILE   = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/></svg>'
      ICON_CALENDAR = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4"><rect x="3" y="4" width="18" height="18" rx="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg>'
      SPARK = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[12px] w-[12px]" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'

      def initialize(profile:, person: nil)
        @profile = profile
        @person  = person || profile.person
      end

      def view_template
        # id is people_details_body (not people_details — that's the turbo-frame id).
        # No people-details Stimulus controller here; the controller lives on the outer
        # wrapper in the conversation partial so it has a pane target.
        div(id: "people_details_body",
            class: "flex flex-col overflow-y-auto text-sm") do
          # Rail close/back header — hidden at xl+
          rail_header

          # Scrollable body
          div(class: "flex-1 px-4 pb-6") do
            identity_section
            scout_read_section
            numbers_section
            threads_section
            documents_section
            events_section
            manage_section
          end
        end
      end

      private

      # ── Rail header ──────────────────────────────────────────────────────────

      def rail_header
        div(class: "flex items-center justify-between border-b border-border px-3 py-2.5 xl:hidden") do
          button(type: "button", class: "inline-flex items-center gap-1 text-[13px] text-accent-600 hover:text-accent-700",
                 data: { action: "click->people-details#close" }) do
            chevron_left_svg
            plain(t(".back"))
          end
          span(class: "text-[13px] font-medium text-foreground") { t(".title") }
          span(class: "w-8") { }
        end
      end

      # ── 1. Identity ──────────────────────────────────────────────────────────

      def identity_section
        div(class: "pt-4") do
          # Avatar + name
          div(class: "flex items-start gap-3 mb-3") do
            render(ContactAvatar.new(email: primary_email.to_s, size: :xl, variant: :neutral,
                                    contact_id: @profile.primary_contact&.id))
            div(class: "min-w-0 flex-1") do
              # Inline rename: span + pencil, swaps to a form
              div(class: "flex items-center gap-1", data: { controller: "inline-rename" }) do
                span(class: "text-[15px] font-semibold text-foreground leading-snug",
                     data: { "inline-rename-target": "display" }) { @person.display_name }
                button(type: "button",
                       class: "mt-0.5 flex-shrink-0 text-muted-foreground/60 hover:text-muted-foreground transition-colors",
                       data: { action: "click->inline-rename#activate", "inline-rename-target": "editBtn" }) do
                  raw(safe(ICON_PENCIL))
                end
                form(action: helpers.rename_people_details_path(@person), method: "patch",
                     class: "hidden items-center gap-1.5 w-full",
                     data: { "inline-rename-target": "form", turbo_stream: true,
                             action: "submit->inline-rename#submit keydown.esc->inline-rename#cancel" }) do
                  input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
                  input(type: "text", name: "name",
                        value: @person.display_name,
                        class: "flex-1 min-w-0 rounded-md border border-border px-2 py-0.5 text-[14px] focus:outline-none focus:ring-1 focus:ring-ring",
                        data: { "inline-rename-target": "input" },
                        autofocus: true,
                        "aria-label": t(".rename_label"))
                  button(type: "submit", class: "text-[11px] font-medium text-primary hover:underline") do
                    t(".save")
                  end
                end
              end

              # Org link
              if (org = @profile.organization)
                a(href: helpers.people_organization_path(org.id),
                  data: { turbo_frame: "_top" },
                  class: "text-[12px] text-muted-foreground hover:text-foreground no-underline truncate block") do
                  plain(org.name)
                end
              end

              # Meta line — address count only when > 1, then first-contact date.
              meta_parts = []
              if @profile.emails.size > 1
                meta_parts << t(".addresses", count: @profile.emails.size)
              end
              if @profile.counts[:first_contact_at]
                meta_parts << t(".since", date: helpers.l(@profile.counts[:first_contact_at], format: :month_year))
              end
              p(class: "text-[11.5px] text-muted-foreground mt-0.5 truncate") { plain(meta_parts.join(" · ")) }
            end
          end

          # Relationship select chip + kind segmented control
          div(class: "flex flex-wrap items-center gap-2 mb-2") do
            relationship_chip
            kind_control
          end

          # Sender tags
          if @profile.tags.any?
            div(class: "flex flex-wrap gap-1.5 mb-2") do
              @profile.tags.each do |tag|
                span(class: "inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium",
                     style: "color: #{tag.color}; background-color: #{tag.color}1f;") { plain(tag.name) }
              end
            end
          end

          # State chips
          div(class: "flex flex-wrap gap-1.5 mb-2") do
            if @profile.starred?
              span(class: "#{CHIP_BASE} bg-amber-50 text-amber-700 dark:bg-amber-900/20 dark:text-amber-400") { t(".starred") }
            end
            case @profile.list_status
            when :blocked
              span(class: "#{CHIP_BASE} bg-destructive/10 text-destructive") { t(".blocked") }
            when :allowed
              span(class: "#{CHIP_BASE} bg-emerald-50 text-emerald-700 dark:bg-emerald-900/20 dark:text-emerald-400") { t(".allowed") }
            end
          end

          # Email addresses
          div(class: "mt-1.5 space-y-1") do
            @profile.emails.each do |(addr, primary)|
              div(class: "flex items-center gap-2 group/addr") do
                span(class: class_names("text-[12px] font-mono truncate flex-1",
                                        primary ? "text-foreground" : "text-muted-foreground")) { plain(addr) }
                span(class: "text-[10px] text-muted-foreground/60 flex-shrink-0") { t(".primary_badge") } if primary
                button(type: "button",
                       class: "flex-shrink-0 text-muted-foreground/40 hover:text-muted-foreground opacity-0 group-hover/addr:opacity-100 transition-opacity",
                       data: { action: "click->copy#copy", "copy-text-param": addr },
                       title: t(".copy_email")) do
                  raw(safe(ICON_COPY))
                end
              end
            end
          end
        end
      end

      def relationship_chip
        # Form has auto-submit controller so the select submits on change.
        form(action: helpers.relationship_people_details_path(@person), method: "patch",
             data: { controller: "auto-submit", turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          select(name: "relationship_type",
                 class: "appearance-none cursor-pointer rounded-full border border-border px-2 py-0.5 text-[11.5px] font-medium text-muted-foreground bg-transparent hover:bg-secondary focus:outline-none",
                 data: { action: "change->auto-submit#submit" }) do
            option(value: "", selected: @profile.relationship.blank?) { t(".no_relationship") }
            Person::RELATIONSHIP_TYPES.each do |rt|
              option(value: rt, selected: @profile.relationship == rt) do
                plain(t("activerecord.attributes.person.relationship_types.#{rt}", default: rt.humanize))
              end
            end
          end
        end
      end

      def kind_control
        return unless @profile.primary_contact

        is_service = @profile.sender_kind == "service"
        # Two submit buttons — one per kind. The active one has the filled style;
        # no hidden input, no invisible overlay, no nonexistent kind-toggle controller.
        form(action: helpers.kind_people_details_path(@person), method: "patch",
             data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          div(class: "inline-flex rounded-full border border-border text-[11px] font-medium overflow-hidden") do
            button(type: "submit", name: "kind", value: "person",
                   class: class_names("px-2.5 py-0.5 transition-colors",
                                      !is_service ? "bg-foreground text-background" : "text-muted-foreground hover:bg-secondary"),
                   "aria-pressed": (!is_service).to_s) do
              t(".kind_person")
            end
            button(type: "submit", name: "kind", value: "service",
                   class: class_names("px-2.5 py-0.5 transition-colors",
                                      is_service ? "bg-foreground text-background" : "text-muted-foreground hover:bg-secondary"),
                   "aria-pressed": is_service.to_s) do
              t(".kind_service")
            end
          end
        end
      end

      # ── 2. Scout's read ──────────────────────────────────────────────────────

      def scout_read_section
        div do
          section_heading(t(".sections.scout_read"))

          if @profile.read.present?
            p(class: "text-[12.5px] text-foreground/90 leading-relaxed mb-2") { plain(@profile.read) }

            if (pats = @profile.patterns).present?
              div(class: "flex flex-wrap gap-1.5 mb-2") do
                pats.each do |key, value|
                  next unless value.present?

                  label = t(".patterns.#{key}", default: key.humanize)
                  if key == "topics" && value.is_a?(Array)
                    value.first(3).each do |topic|
                      span(class: "#{CHIP_BASE} bg-secondary text-muted-foreground") { plain(topic.to_s) }
                    end
                  else
                    span(class: "#{CHIP_BASE} bg-secondary text-muted-foreground") do
                      plain("#{label} · #{value}")
                    end
                  end
                end
              end
            end
          else
            p(class: "text-[12.5px] text-muted-foreground italic") { t(".scout_not_read") }
          end

          div(class: "flex items-center justify-between") do
            if @profile.analyzed_at
              span(class: "text-[11px] text-muted-foreground") do
                t(".read_on", date: helpers.l(@profile.analyzed_at.to_date, format: :day_month))
              end
            end
            refresh_form
          end
        end
      end

      def refresh_form
        form(action: helpers.analyze_people_details_path(@person), method: "post",
             data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit",
                 class: "inline-flex items-center gap-1 text-[11.5px] text-muted-foreground hover:text-foreground transition-colors",
                 title: t(".refresh_hint")) do
            raw(safe(ICON_REFRESH))
            plain(t(".refresh"))
          end
        end
      end

      # ── 3. Numbers ───────────────────────────────────────────────────────────

      def numbers_section
        div do
          section_heading(t(".sections.numbers"))
          div(class: "grid grid-cols-2 gap-x-4 gap-y-1.5") do
            fact_row(t(".numbers.received"),   @profile.counts[:received].to_s)
            fact_row(t(".numbers.sent"),       @profile.counts[:sent].to_s)
            fact_row(t(".numbers.threads"),    @profile.counts[:threads].to_s)
            fact_row(t(".numbers.documents"),  @profile.counts[:documents].to_s)
            if (first = @profile.counts[:first_contact_at])
              fact_row(t(".numbers.first_contact"), helpers.l(first.to_date, format: :day_month))
            end
            if (last = @profile.counts[:last_contact_at])
              fact_row(t(".numbers.last_contact"), helpers.l(last.to_date, format: :day_month))
            end
          end
        end
      end

      def fact_row(label, value)
        div(class: "flex items-baseline justify-between gap-2") do
          span(class: FACT_LABEL) { plain(label) }
          span(class: FACT_VALUE) { plain(value) }
        end
      end

      # ── 4. Threads ───────────────────────────────────────────────────────────

      def threads_section
        return if @profile.threads.empty?

        div do
          section_heading("#{t(".sections.threads")} (#{@profile.counts[:threads]})")
          div(class: "space-y-0.5") do
            @profile.threads.each do |thread|
              thread_row(thread)
            end
          end
          if @profile.more_threads?
            p(class: "mt-1.5 text-[11px] text-muted-foreground") do
              t(".threads_more", count: @profile.counts[:threads] - People::Profile::THREAD_CAP)
            end
          end
        end
      end

      def thread_row(thread)
        a(href: helpers.person_page_path(@person, thread: thread[:id]),
          data: { turbo_frame: "people_detail", action: "click->people-details#jump",
                  "people-details-thread-id-param": thread[:id] },
          class: "flex items-start gap-2 rounded-lg px-1.5 py-1 no-underline hover:bg-secondary transition-colors group/thread") do
          div(class: "min-w-0 flex-1") do
            p(class: "text-[12.5px] text-foreground truncate") { plain(thread[:subject].presence || t("components.people.thread_block.no_subject")) }
            span(class: "text-[11px] text-muted-foreground") do
              plain(helpers.pluralize(thread[:count], t(".thread_message_one"), plural: t(".thread_messages_other")))
            end
          end
          if (at = thread[:latest_at])
            span(class: "flex-shrink-0 text-[11px] text-muted-foreground") { plain(format_date(at)) }
          end
        end
      end

      # ── 5. Documents ─────────────────────────────────────────────────────────

      def documents_section
        return if @profile.documents.empty?

        div do
          div(class: "flex items-center justify-between") do
            section_heading("#{t(".sections.documents")} (#{@profile.counts[:documents]})")
            a(href: paper_link,
              data: { turbo_frame: "_top" },
              class: "text-[11px] text-muted-foreground hover:text-foreground no-underline") { t(".all_in_paper") }
          end
          div(class: "space-y-0.5") do
            @profile.documents.each do |doc|
              document_row(doc)
            end
          end
        end
      end

      def document_row(doc)
        facts  = Documents::Facts.for(doc)
        status = Documents::Status.for(doc)
        has_amount = facts.segments.any?(&:emphasis)

        a(href: helpers.document_path(doc),
          data: { turbo_frame: "_top" },
          class: "flex items-start gap-2.5 rounded-lg px-1.5 py-1.5 no-underline hover:bg-secondary transition-colors") do
          # Icon
          span(class: "flex-shrink-0 inline-flex h-[28px] w-[28px] items-center justify-center rounded-md bg-secondary text-muted-foreground") do
            raw(safe(has_amount ? ICON_MONEY : ICON_FILE))
          end
          div(class: "min-w-0 flex-1") do
            p(class: "text-[12.5px] font-medium text-foreground truncate leading-snug") { plain(doc.display_title) }
            div(class: "flex items-center gap-1 flex-wrap mt-0.5") do
              span(class: "text-[11px] text-muted-foreground") { plain(doc.classification&.name || t(".unknown_kind")) }
              if status
                span(class: "text-muted-foreground/50 text-[11px]") { "·" }
                render(Campbooks::StatusChip.for(status))
              end
            end
          end
          div(class: "flex-shrink-0 text-right") do
            if (amount_seg = facts.segments.find(&:emphasis))
              p(class: "text-[12px] font-semibold tabular-nums text-foreground") { plain(amount_seg.text) }
            end
            p(class: "text-[11px] text-muted-foreground tabular-nums") do
              plain(helpers.l(doc.created_at.to_date, format: :day_month))
            end
          end
        end
      end

      # ── 6. Events ────────────────────────────────────────────────────────────

      def events_section
        return if @profile.events.empty?

        div do
          div(class: "flex items-center justify-between") do
            section_heading("#{t(".sections.events")} (#{@profile.events.size})")
            a(href: helpers.time_path,
              data: { turbo_frame: "_top" },
              class: "text-[11px] text-muted-foreground hover:text-foreground no-underline") { t(".open_time") }
          end
          div(class: "space-y-0.5") do
            @profile.events.each { |ev| event_row(ev) }
          end
        end
      end

      def event_row(ev)
        a(href: helpers.calendar_event_path(ev),
          data: { turbo_frame: "_top" },
          class: "flex items-start gap-2.5 rounded-lg px-1.5 py-1.5 no-underline hover:bg-secondary transition-colors") do
          span(class: "flex-shrink-0 inline-flex h-[28px] w-[28px] items-center justify-center rounded-md bg-secondary text-muted-foreground") do
            raw(safe(ICON_CALENDAR))
          end
          div(class: "min-w-0 flex-1") do
            p(class: "text-[12.5px] font-medium text-foreground truncate") { plain(ev.title.presence || t(".no_event_title")) }
            span(class: "text-[11px] text-muted-foreground") do
              parts = []
              parts << helpers.l(ev.start_at.to_date, format: :day_month) if ev.start_at
              # Attendees: skip self (profile's own emails) and user's own account addresses.
              person_emails_set = @profile.emails.map { |addr, _| addr.downcase }.to_set
              attendee_names = Array(ev.attendees)
                .reject { |a| person_emails_set.include?(a["email"].to_s.downcase) }
                .first(2)
                .filter_map { |a| a["name"].presence || a["email"].presence }
              parts << attendee_names.join(", ") if attendee_names.any?
              plain(parts.join(" · "))
            end
          end
        end
      end

      # ── 7. Manage ────────────────────────────────────────────────────────────

      def manage_section
        div(class: "pt-5 border-t border-border mt-4 space-y-2") do
          # Merge suggestion
          if (dup = @profile.duplicate_suggestion)
            merge_card(dup)
          end

          # Star / Unstar
          star_action = @profile.starred? ? :unstar : :star
          star_label  = @profile.starred? ? t(".manage.unstar") : t(".manage.star")
          state_form(star_action, star_label)

          # Allow (only when not already allowed)
          unless @profile.list_status == :allowed || @profile.list_status == :blocked
            state_form(:allow, t(".manage.allow"))
          end

          # Block / Unblock
          if @profile.list_status == :blocked
            state_form(:unblock, t(".manage.unblock"))
          else
            state_form(:block, t(".manage.block"), confirm: t(".manage.block_confirm"))
          end

          # Organization link
          if (org = @profile.organization)
            a(href: helpers.people_organization_path(org.id),
              data: { turbo_frame: "_top" },
              class: "#{ACTION_BTN} w-full justify-center no-underline") do
              plain(t(".manage.open_org", org: org.name))
            end
          end
        end
      end

      def merge_card(dup)
        div(class: "rounded-xl border border-border p-3 mb-2") do
          div(class: "flex items-start gap-2 mb-2") do
            span(style: "color: var(--ember-solid)") { raw(safe(SPARK)) }
            div do
              p(class: "text-[12.5px] font-medium text-foreground") { t(".manage.same_person") }
              p(class: "text-[12px] text-muted-foreground mt-0.5") { plain(dup[:reason].to_s) }
            end
          end
          form(action: helpers.merge_people_details_path(@person), method: "post",
               data: { turbo_stream: true }) do
            input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
            input(type: "hidden", name: "approve", value: "true")
            button(type: "submit",
                   class: "#{ACTION_BTN} w-full justify-center text-primary border-primary/40 hover:bg-primary/5") do
              t(".manage.merge", name: dup[:name])
            end
          end
        end
      end

      def state_form(state, label, confirm: nil)
        form(action: helpers.state_people_details_path(@person), method: "patch",
             data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "state", value: state)
          button(type: "submit",
                 class: "#{ACTION_BTN} w-full justify-center",
                 **(confirm ? { data: { "turbo-confirm": confirm } } : {})) do
            plain(label)
          end
        end
      end

      # ── Helpers ──────────────────────────────────────────────────────────────

      def section_heading(text)
        div(class: SECTION_H) { plain(text) }
      end

      def primary_email
        @profile.emails.find { |_addr, primary| primary }&.first || @profile.primary_contact&.email.to_s
      end

      def paper_link
        if primary_email.present?
          helpers.paper_path(q: primary_email)
        else
          helpers.paper_path
        end
      end

      def format_date(time)
        if time.to_date == Date.current
          helpers.l(time, format: :clock)
        elsif time.year == Date.current.year
          helpers.l(time.to_date, format: :day_month)
        else
          helpers.l(time.to_date, format: :date)
        end
      end

      def chevron_left_svg
        raw(safe('<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/></svg>'))
      end
    end
  end
end
