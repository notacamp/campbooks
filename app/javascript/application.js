// Configure your import map in config/importmap.rb. Read more: https://github.com/rails/importmap-rails
import "@hotwired/turbo-rails"
import { createConsumer } from "@rails/actioncable"

window.App = window.App || {}
window.App.cable = createConsumer()

import "controllers"
