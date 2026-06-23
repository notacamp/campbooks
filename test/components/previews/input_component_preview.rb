class InputComponentPreview < ViewComponent::Preview
  # Default text input with label and placeholder
  def default
    render Campbooks::Input.new("name", label: "Full name", placeholder: "Enter your name")
  end

  # Email input matching the sign-in form style
  def email
    render Campbooks::Input.new("email_address", type: :email, label: "Email address",
           placeholder: "you@example.com", required: true)
  end

  # Password input with hint text
  def password
    render Campbooks::Input.new("password", type: :password, label: "Password",
           placeholder: "Enter your password", hint: "Must be at least 8 characters")
  end

  # Input with an error message and red border
  def with_error
    render Campbooks::Input.new("email", type: :email, label: "Email address",
           value: "not-an-email", error: "Please enter a valid email address")
  end

  # Disabled input showing grayed-out state
  def disabled
    render Campbooks::Input.new("username", label: "Username", value: "johndoe", disabled: true)
  end

  # Inline/filter style using rounded-md
  def inline
    render Campbooks::Input.new("search", placeholder: "Search...", rounded: :md)
  end

  # Number input
  def number
    render Campbooks::Input.new("quantity", type: :number, label: "Quantity", value: "1")
  end

  # Date input
  def date
    render Campbooks::Input.new("date", type: :date, label: "Date")
  end
end
