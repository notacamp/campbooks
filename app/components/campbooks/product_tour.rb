# frozen_string_literal: true

module Campbooks
  # Walkthrough v2: an explanation-first, stories-style slideshow that teaches
  # Campbooks by showing what each module does rather than asking the user to
  # interact. Six slides: (1) what Campbooks is with rotating Scout statements,
  # (2) Inbox, (3) Calendar, (4) Tasks (feature-gated), (5) Documents, (6) "much
  # more" + docs + connect CTA.
  #
  # Segmented progress bars (tap a segment to jump), module label + n/N counter,
  # slide enter/exit transitions, and per-slide vignette animations are all driven
  # by the `product-tour` Stimulus controller. No task-gating — slides are
  # informational and Next is always enabled.
  #
  # On finish/skip it POSTs the one-time tour flag (User#dismiss_tour!, key
  # "product_tour") so it only auto-opens once; "Take the tour" and ?tour=1
  # re-open it on demand.
  class ProductTour < Campbooks::Base
    # Public docs site — override DOCS_URL env to point at your own.
    DOCS_URL = ENV.fetch("DOCS_URL", "https://campbooks.not-a-camp.com/docs").freeze

    # Five rotating Scout statements on slide 1 (index 0–4).
    ROTATION_COUNT = 5

    # @param autostart [Boolean] auto-open on connect (first run on home page)
    # @param connect_path [String, nil] ignored; accepted for backward-compat with
    #   layouts that pass it (layouts/onboarding). The connect path is now determined
    #   dynamically by user_has_active_inbox?.
    def initialize(autostart: false, connect_path: nil, **attrs) # rubocop:disable Lint/UnusedMethodArgument
      @autostart = autostart
      @attrs = attrs
    end

    # The ordered list of slide keys for this render. Tasks is omitted when the
    # feature flag is off so segments + counter adapt automatically.
    def slides
      @slides ||= begin
        list = %i[intro inbox calendar docs more]
        list.insert(3, :tasks) if Features.tasks?
        list
      end
    end

    def view_template
      div(
        class: "hidden fixed inset-0 z-[70] items-center justify-center bg-background/95 backdrop-blur-sm p-4",
        role: "dialog", aria_modal: "true", aria_label: t(".aria_label"),
        tabindex: "-1",
        data: {
          product_tour_target: "panel",
          tour_autostart: @autostart.to_s,
          action: "keydown->product-tour#onKeydown"
        },
        **@attrs
      ) do
        div(
          # A real height (not just max-h): the slides area is flex-1 with
          # absolutely-positioned slides inside, so without a definite height
          # the card collapses to its chrome and every slide renders 0-tall.
          class: "w-full max-w-[620px] bg-background border border-border rounded-[26px] shadow-2xl flex flex-col p-6 sm:p-8 h-[min(88dvh,760px)]"
        ) do
          progress_bars
          deck_head
          slides_area
          deck_nav
        end
      end
    end

    private

    # ── Progress bars ────────────────────────────────────────────────────────

    def progress_bars
      div(class: "flex gap-1.5", role: "tablist", aria_label: t(".progress_label")) do
        slides.each_with_index do |_key, i|
          button(
            type: "button",
            role: "tab",
            class: "tour-seg-btn flex-1 relative h-[3.5px] rounded-full bg-subtle border-0 p-0 cursor-pointer overflow-hidden focus-visible:outline-2 focus-visible:outline-foreground focus-visible:outline-offset-2",
            data: {
              product_tour_target: "segment",
              action: "click->product-tour#goTo",
              tour_segment_index: i.to_s
            },
            aria_label: t(".segment_label", n: i + 1)
          ) do
            span(
              class: "tour-seg-fill absolute inset-0 rounded-full bg-foreground origin-left scale-x-0",
              data: { product_tour_target: "segFill" }
            )
          end
        end
      end
    end

    # ── Deck head: module label (left) + counter + skip (right) ─────────────

    def deck_head
      div(class: "flex items-center justify-between mt-3.5 min-h-5") do
        span(
          class: "text-[11px] font-bold tracking-[0.12em] uppercase text-muted-foreground flex items-center gap-1.5",
          data: { product_tour_target: "modLabel" }
        )
        div(class: "flex items-center gap-3") do
          span(
            class: "text-[11.5px] text-muted-foreground tabular-nums",
            data: { product_tour_target: "countLabel" }
          )
          button(
            type: "button",
            class: "text-xs font-medium text-muted-foreground hover:text-foreground transition-colors px-1.5 py-0.5 rounded-md",
            data: { action: "click->product-tour#skip" }
          ) { t(".skip") }
        end
      end
    end

    # ── Slides container ─────────────────────────────────────────────────────

    def slides_area
      # relative + flex-1 so slides fill the available height;
      # min-h-0 lets the flex-child shrink below its natural height.
      div(class: "relative flex-1 mt-3 min-h-0") do
        slides.each_with_index do |key, i|
          div(
            class: "tour-slide absolute inset-0 flex flex-col overflow-y-auto",
            data: {
              product_tour_target: "slide",
              tour_slide_type: key.to_s,
              tour_slide_index: i.to_s,
              tour_mod_label: t(".#{key}_mod"),
              tour_mod_icon: key.to_s
            }
          ) do
            case key
            when :intro    then slide_intro
            when :inbox    then slide_inbox
            when :calendar then slide_calendar
            when :tasks    then slide_tasks
            when :docs     then slide_docs
            when :more     then slide_more
            end
          end
        end
      end
    end

    # ── Navigation ───────────────────────────────────────────────────────────

    def deck_nav
      div(class: "flex items-center justify-between mt-4 pt-0 gap-3") do
        button(
          type: "button",
          class: "w-9 h-9 rounded-full border border-border flex items-center justify-center text-[18px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-30 disabled:pointer-events-none leading-none",
          data: { product_tour_target: "prevBtn", action: "click->product-tour#prev" },
          aria_label: t(".prev"),
          disabled: true
        ) { raw safe("&#8249;") }

        span(class: "text-[11.5px] text-muted-foreground") { t(".nav_hint") }

        button(
          type: "button",
          class: "w-9 h-9 rounded-full border border-border flex items-center justify-center text-[18px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground leading-none",
          data: { product_tour_target: "nextBtn", action: "click->product-tour#next" },
          aria_label: t(".next")
        ) { raw safe("&#8250;") }
      end
    end

    # ── Slide 1: Intro — what Campbooks is ───────────────────────────────────

    def slide_intro
      # Ember mark
      div(class: "flex-shrink-0") do
        div(
          class: "w-14 h-14 rounded-[17px] bg-ember-gradient shadow-ember flex items-center justify-center text-white text-[22px] font-bold"
        ) { raw safe(spark_svg("h-7 w-7")) }
      end

      h2(class: "text-[22px] sm:text-[26px] font-bold tracking-[-0.025em] mt-5 leading-[1.2] text-balance") do
        t(".s1_title")
      end

      p(class: "mt-2.5 text-[14.5px] leading-relaxed text-muted-foreground max-w-[42ch]") do
        t(".s1_sub")
      end

      # Rotating Scout statements
      div(class: "mt-6") do
        span(class: "text-[12px] font-[650] tracking-[0.1em] uppercase text-muted-foreground flex items-center gap-2") do
          # Scout avatar dot
          span(class: "w-[22px] h-[22px] rounded-full bg-ember-gradient shadow-ember flex-shrink-0 flex items-center justify-center text-white") do
            raw safe(spark_svg("h-2.5 w-2.5"))
          end
          plain t(".s1_rot_lead")
        end

        div(class: "relative mt-2.5", style: "min-height: 96px;") do
          ROTATION_COUNT.times do |i|
            div(
              class: "tour-rot-item absolute inset-0",
              data: { tour_rot_item: i.to_s }
            ) do
              h3(class: "text-[18px] sm:text-[20px] font-[650] tracking-[-0.02em] leading-[1.3]") do
                t(".s1_r#{i}_h")
              end
              p(class: "mt-1.5 text-[13px] text-muted-foreground max-w-[46ch]") do
                t(".s1_r#{i}_p")
              end
            end
          end
        end
      end

      # Closing line — pushed to the bottom
      p(class: "mt-auto pt-4 text-[13px] text-muted-foreground") do
        b(class: "text-foreground") { t(".s1_close_b") }
        plain " "
        plain t(".s1_close")
      end
    end

    # ── Slide 2: Inbox ───────────────────────────────────────────────────────

    def slide_inbox
      feature_head(key: "s2")

      div(class: "mt-5 min-w-0") do
        # Row 1: Bank email with tag chip
        div(class: "tour-stage-el flex items-center gap-2.5 py-2.5 border-t border-border") do
          div(class: "w-7 h-7 rounded-full bg-subtle flex items-center justify-center text-[11px] font-semibold text-muted-foreground flex-shrink-0") { plain "F" }
          div(class: "min-w-0 flex-1") do
            div(class: "flex items-baseline justify-between gap-2") do
              span(class: "truncate text-[13px] font-[550]") { plain "First National Bank" }
              span(class: "shrink-0 text-[11px] text-muted-foreground tabular-nums") { plain "9:14 AM" }
            end
            div(class: "mt-0.5 text-[12.5px] text-muted-foreground truncate") { plain "Transfer confirmation needed" }
          end
          span(
            class: "tour-chip inline-flex items-center rounded-lg bg-muted text-muted-foreground text-[11px] font-semibold px-2 py-0.5 flex-shrink-0",
            style: "opacity: 0;",
            data: { tour_chip: "true" }
          ) { plain "#important" }
          span(class: "w-1.5 h-1.5 rounded-full flex-shrink-0", style: "background-color: var(--ember-solid)")
        end

        # Row 2: Jordan email with tag chip
        div(class: "tour-stage-el flex items-center gap-2.5 py-2.5 border-t border-border") do
          div(class: "w-7 h-7 rounded-full bg-subtle flex items-center justify-center text-[11px] font-semibold text-muted-foreground flex-shrink-0") { plain "J" }
          div(class: "min-w-0 flex-1") do
            div(class: "flex items-baseline justify-between gap-2") do
              span(class: "truncate text-[13px] font-[550]") { plain "Jordan Lee" }
              span(class: "shrink-0 text-[11px] text-muted-foreground tabular-nums") { plain "8:55 AM" }
            end
            div(class: "mt-0.5 text-[12.5px] text-muted-foreground truncate") { plain "Q3 project proposal — revised" }
          end
          span(
            class: "tour-chip inline-flex items-center rounded-lg bg-muted text-muted-foreground text-[11px] font-semibold px-2 py-0.5 flex-shrink-0",
            style: "opacity: 0;",
            data: { tour_chip: "true" }
          ) { plain "#needs reply" }
        end

        # Group row: notifications bundle
        div(class: "tour-stage-el flex items-center gap-2.5 py-2.5 border-t border-b border-border text-muted-foreground text-[13px]") do
          span(class: "w-7 h-7 rounded-full bg-muted flex items-center justify-center text-[13px] flex-shrink-0") { plain "🔔" }
          plain t(".s2_group_label")
          span(
            class: "ml-auto bg-subtle rounded-full px-2 py-0.5 text-[11px] font-[650] text-muted-foreground tabular-nums",
            data: { tour_counter: "true", tour_counter_max: "12" }
          ) { plain "0" }
        end
      end
    end

    # ── Slide 3: Calendar ────────────────────────────────────────────────────

    def slide_calendar
      feature_head(key: "s3")

      div(class: "mt-5 space-y-2.5 min-w-0") do
        # Event block
        div(class: "tour-stage-el border border-border rounded-[14px] bg-card p-3 flex items-center gap-3 flex-wrap") do
          span(class: "w-2.5 h-2.5 rounded-[3px] flex-shrink-0", style: "background-color: oklch(48% 0.14 248)")
          div(class: "flex-1 min-w-0") do
            b(class: "block text-[13.5px] font-semibold") { plain t(".s3_ev_title") }
            small(class: "text-[12px] text-muted-foreground tabular-nums") { plain t(".s3_ev_time") }
          end
          div(class: "ml-auto") do
            button(
              type: "button",
              tabindex: "-1",
              aria_disabled: "true",
              class: "tour-morph-btn text-[12px] font-[550] rounded-[9px] px-2.5 py-1.5 cursor-default bg-foreground text-background border-0 font-semibold",
              data: { tour_morph_at: "3200", tour_morph_text: t(".s3_ev_added"), tour_original_text: t(".s3_ev_add") }
            ) { plain t(".s3_ev_add") }
          end
        end

        # Reminder row
        div(class: "tour-stage-el flex items-center gap-2.5 py-2.5 border-t border-border") do
          span(class: "w-5 h-5 rounded-full bg-ember-gradient shadow-ember flex items-center justify-center text-white flex-shrink-0") do
            raw safe(spark_svg("h-2.5 w-2.5"))
          end
          div(class: "flex-1 min-w-0") do
            div(class: "text-[13px] font-[550] truncate") { plain t(".s3_rem_sender") }
            div(class: "text-[12px] text-muted-foreground truncate") { plain t(".s3_rem_subj") }
          end
          button(
            type: "button",
            tabindex: "-1",
            aria_disabled: "true",
            class: "tour-morph-btn ml-2 text-[12px] font-[550] rounded-[9px] px-2.5 py-1.5 cursor-default bg-foreground text-background border-0 font-semibold flex-shrink-0",
            data: { tour_morph_at: "4600", tour_morph_text: t(".s3_rem_added"), tour_original_text: t(".s3_rem_add") }
          ) { plain t(".s3_rem_add") }
        end
      end
    end

    # ── Slide 4: Tasks (feature-gated) ───────────────────────────────────────

    def slide_tasks
      feature_head(key: "s4")

      div(class: "mt-5 min-w-0") do
        # Task 1 — ticks and strikes after 2600ms
        div(class: "tour-stage-el flex items-center gap-2.5 py-2.5 border-t border-border") do
          span(
            class: "w-[19px] h-[19px] rounded-[7px] border-[1.5px] border-border flex items-center justify-center text-[11px] bg-background flex-shrink-0",
            data: { tour_tick: "true" }
          )
          span(
            class: "text-[13.5px] font-[550] flex-1 min-w-0",
            data: { tour_tick_text: "true" }
          ) { plain "Send Q3 report to Jordan" }
          span(class: "inline-flex items-center gap-1 rounded-lg border px-2 py-0.5 text-[11px] font-semibold flex-shrink-0", style: "background-color: oklch(97% 0.03 85); border-color: oklch(90% 0.06 85); color: oklch(52% 0.12 78)") do
            plain "🕒 Friday"
          end
        end

        # Task 2
        div(class: "tour-stage-el flex items-center gap-2.5 py-2.5 border-t border-border") do
          span(class: "w-[19px] h-[19px] rounded-[7px] border-[1.5px] border-border flex items-center justify-center text-[11px] bg-background flex-shrink-0")
          span(class: "text-[13.5px] font-[550] flex-1 min-w-0") { plain "Confirm bank transfer to Acme Corp" }
          span(class: "inline-flex items-center gap-1 rounded-lg border px-2 py-0.5 text-[11px] font-semibold flex-shrink-0", style: "background-color: oklch(97% 0.03 85); border-color: oklch(90% 0.06 85); color: oklch(52% 0.12 78)") do
            plain "🕒 Today, 5 PM"
          end
        end

        # Task 3
        div(class: "tour-stage-el flex items-center gap-2.5 py-2.5 border-t border-b border-border") do
          span(class: "w-[19px] h-[19px] rounded-[7px] border-[1.5px] border-border flex items-center justify-center text-[11px] bg-background flex-shrink-0")
          span(class: "text-[13.5px] font-[550] flex-1 min-w-0") { plain "Countersign the Oakwood contract" }
          span(class: "inline-flex items-center gap-1 rounded-lg border px-2 py-0.5 text-[11px] font-semibold flex-shrink-0", style: "background-color: oklch(97% 0.03 85); border-color: oklch(90% 0.06 85); color: oklch(52% 0.12 78)") do
            plain "🕒 Thursday"
          end
        end
      end
    end

    # ── Slide 5: Documents ───────────────────────────────────────────────────

    def slide_docs
      feature_head(key: "s5")

      div(class: "mt-5 space-y-2.5 min-w-0") do
        # Attachment chip
        span(class: "tour-stage-el inline-flex items-center gap-1.5 border border-border rounded-lg bg-muted text-muted-foreground text-[11.5px] font-[550] px-2 py-1") do
          plain "📎 invoice-10-449.pdf · 84 KB"
        end

        # Document card
        div(class: "tour-stage-el border border-border rounded-[16px] bg-card p-3.5") do
          # Card head: type pill + confidence bar
          div(class: "flex items-center gap-2 flex-wrap") do
            span(class: "inline-flex items-center gap-1.5 rounded-full bg-muted px-2.5 py-1 text-[11.5px] font-[650]") do
              span(class: "w-2 h-2 rounded-full", style: "background-color: oklch(53% 0.16 48)")
              plain t(".s5_doc_type")
            end
            div(class: "flex items-center gap-1.5 ml-auto") do
              div(class: "w-[52px] h-[6px] rounded-full bg-muted overflow-hidden") do
                div(class: "h-full rounded-full", style: "width: 86%; background-color: oklch(62% 0.18 150)")
              end
              small(class: "text-[11px] text-muted-foreground tabular-nums") { plain "86%" }
            end
          end

          # Extracted fields — fade in one by one
          div(class: "grid gap-2.5 mt-3", style: "grid-template-columns: repeat(auto-fit, minmax(100px, 1fr))") do
            [
              [ t(".s5_field_vendor_label"), "Streamline SaaS" ],
              [ t(".s5_field_amount_label"), "$49.00" ],
              [ t(".s5_field_due_label"), "Oct 15, 2026" ]
            ].each_with_index do |(label, val), i|
              div(
                class: "tour-doc-field",
                style: "opacity: 0;",
                data: { tour_field_index: i.to_s }
              ) do
                small(class: "block text-[10px] font-[650] tracking-[0.1em] uppercase text-muted-foreground") { plain label }
                b(class: "text-[13px] font-semibold tabular-nums") { plain val }
              end
            end
          end

          # Actions
          div(class: "flex justify-end gap-2 mt-3") do
            button(
              type: "button",
              tabindex: "-1",
              aria_disabled: "true",
              class: "text-[12px] font-[550] rounded-[9px] px-2.5 py-1.5 cursor-default border border-border bg-transparent text-muted-foreground"
            ) { plain t(".s5_reclassify") }
            button(
              type: "button",
              tabindex: "-1",
              aria_disabled: "true",
              class: "tour-morph-btn text-[12px] font-semibold rounded-[9px] px-2.5 py-1.5 cursor-default bg-foreground text-background border-0",
              data: { tour_morph_at: "3600", tour_morph_text: t(".s5_approved"), tour_original_text: t(".s5_approve") }
            ) { plain t(".s5_approve") }
          end
        end
      end
    end

    # ── Slide 6: Much more ───────────────────────────────────────────────────

    def slide_more
      h2(class: "text-[20px] sm:text-[22px] font-bold tracking-[-0.02em] leading-[1.25] text-balance") do
        t(".s6_title")
      end

      p(class: "tour-stage-el mt-2.5 text-[13.5px] leading-[1.65] text-muted-foreground max-w-[56ch]") do
        t(".s6_body")
      end

      div(class: "tour-stage-el flex flex-wrap gap-2 mt-4") do
        [
          t(".s6_chip_skim"), t(".s6_chip_scout"), t(".s6_chip_rules"),
          t(".s6_chip_digests"), t(".s6_chip_templates"), t(".s6_chip_api"),
          t(".s6_chip_self")
        ].each do |label|
          span(class: "border border-border rounded-full px-3 py-1.5 text-[12.5px] font-[550] text-muted-foreground") { plain label }
        end
      end

      p(class: "tour-stage-el mt-3 text-[12.5px] text-muted-foreground") { t(".s6_docs_note") }

      # CTAs — pushed to the bottom of the slide
      div(class: "flex flex-wrap gap-2.5 mt-auto pt-5") do
        # Primary CTA: Connect or "back" depending on account state
        if user_has_active_inbox?
          button(
            type: "button",
            class: "flex-1 min-w-[140px] inline-flex items-center justify-center rounded-xl bg-foreground text-background px-4 py-2.5 text-[14px] font-semibold transition-transform active:scale-[0.98]",
            data: { action: "click->product-tour#finishConnect", tour_connect_path: "/" }
          ) { t(".done_cta") }
        else
          button(
            type: "button",
            class: "flex-1 min-w-[140px] inline-flex items-center justify-center rounded-xl bg-foreground text-background px-4 py-2.5 text-[14px] font-semibold transition-transform active:scale-[0.98]",
            data: { action: "click->product-tour#finishConnect", tour_connect_path: "/onboarding" }
          ) { t(".connect_cta") }
        end

        a(
          href: DOCS_URL,
          target: "_blank",
          rel: "noopener noreferrer",
          class: "flex-1 min-w-[120px] inline-flex items-center justify-center rounded-xl border border-border px-4 py-2.5 text-[14px] font-[550] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        ) { t(".docs_cta") }
      end
    end

    # ── Shared sub-components ────────────────────────────────────────────────

    # Eyebrow + title + two body paragraphs (HTML-safe, with <b> highlights).
    def feature_head(key:)
      h2(class: "text-[20px] sm:text-[21px] font-bold tracking-[-0.02em] leading-[1.25] text-balance") do
        t(".#{key}_title")
      end
      p(class: "tour-stage-el mt-2.5 text-[13.5px] leading-[1.65] text-muted-foreground max-w-[56ch]") do
        raw safe(t(".#{key}_body1_html"))
      end
      p(class: "tour-stage-el mt-2 text-[13.5px] leading-[1.65] text-muted-foreground max-w-[56ch]") do
        raw safe(t(".#{key}_body2_html"))
      end
    end

    # ── Helpers ──────────────────────────────────────────────────────────────

    # True when the current user has at least one connected email account.
    # Defensive: returns false if the auth context is unavailable.
    def user_has_active_inbox?
      user = helpers.current_user
      return false unless user
      user.email_accounts.exists?
    rescue
      false
    end

    # ── Icons (inline SVG) ────────────────────────────────────────────────────

    def spark_svg(size = "h-3.5 w-3.5")
      %(<svg viewBox="0 0 24 24" fill="currentColor" class="#{size}" aria-hidden="true"><path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>)
    end
  end
end
