- **The RabbitMQ password can live in a file.** `CAMPBOOKS_TRACKING__AMQP_PASSWORD_FILE`
  points at a file holding the broker password, like `CAMPBOOKS_DATABASE_PASSWORD_FILE`
  for the database: it replaces any password in `CAMPBOOKS_TRACKING__AMQP_URL`, so the
  secret stays out of the environment. With tracked events going to RabbitMQ, a missing
  or empty file stops the start with a message that names the setting.
