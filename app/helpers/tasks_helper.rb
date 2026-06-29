module TasksHelper
  # Class strings that mirror Campbooks::Button (size :sm), for the button_to /
  # link_to actions where the Phlex component can't be embedded directly. Keeps the
  # Tasks UI on the same design-system vocabulary as the calendar and the rest of
  # the app (bg-primary for primary actions, border for outline, etc.).
  # Fixed h-9 so every task action (and the inline status select) lines up on one
  # baseline — the misaligned-heights bug the user hit came from mixing components.
  TASK_BTN_BASE = "cursor-pointer inline-flex h-9 items-center justify-center gap-1.5 px-3 text-sm font-medium " \
                  "whitespace-nowrap select-none transition-[transform,background-color,border-color,color,box-shadow] " \
                  "duration-150 ease-out active:scale-[0.98] rounded-md".freeze

  TASK_BTN_VARIANTS = {
    primary: "bg-primary text-primary-foreground shadow-sm hover:bg-primary/90",
    outline: "border border-input bg-background text-foreground shadow-sm hover:bg-accent hover:text-accent-foreground",
    ghost:   "text-foreground hover:bg-accent hover:text-accent-foreground"
  }.freeze

  def task_btn(variant = :outline)
    "#{TASK_BTN_BASE} #{TASK_BTN_VARIANTS.fetch(variant, TASK_BTN_VARIANTS[:outline])}"
  end
end
